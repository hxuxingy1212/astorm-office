//! 图表部件解析
//!
//! 解析 `ppt/charts/chartN.xml`（DrawingML Chart）为 `ChartElement`。
//! 优先读取缓存数据（`numCache`/`strCache`），无需嵌入工作簿。

use crate::model::elements::{ChartElement, ChartSeries};
use crate::utils::xml::raw_local_name;
use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    None,
    Name,
    Category,
    Value,
}

/// 去掉 `<c:externalData>`（指向嵌入工作簿），使图表自包含、无需额外部件
pub(crate) fn strip_external_data(xml: &str) -> String {
    let mut out = xml.to_string();
    while let Some(start) = out.find("<c:externalData") {
        // 找到该元素的结束：自闭合 "/>" 或配对 "</c:externalData>"
        let after = &out[start..];
        let end_rel = if let Some(close) = after.find("</c:externalData>") {
            close + "</c:externalData>".len()
        } else if let Some(sc) = after.find("/>") {
            sc + 2
        } else {
            break;
        };
        out.replace_range(start..start + end_rel, "");
    }
    out
}

/// 解析图表 XML（位置/名称由调用方按 graphicFrame 设置）
pub(crate) fn parse(xml: &str) -> Option<ChartElement> {
    if xml.is_empty() {
        return None;
    }
    let mut chart = ChartElement::default();
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut plot_depth = 0i32;
    let mut in_ser = false;
    let mut ser = ChartSeries::default();
    let mut mode = Mode::None;
    let mut in_title = false;
    let mut title_text = String::new();
    let mut text_buf = String::new();
    let mut pending_text = false;
    let mut capture_text = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let n = raw_local_name(e.name().as_ref());
                match n.as_str() {
                    "plotArea" => plot_depth += 1,
                    _ if n.ends_with("Chart") && chart.chart_type.is_empty() => {
                        chart.chart_type = base_type(&n).to_string();
                    }
                    "barDir" => {
                        // col/bar 决定柱状图方向
                        if let Some(v) = attr(e, b"val") {
                            if chart.chart_type == "bar" || v == "bar" {
                                chart.chart_type =
                                    if v == "bar" { "bar" } else { "column" }.to_string();
                            }
                        }
                    }
                    "ser" => {
                        in_ser = true;
                        ser = ChartSeries::default();
                    }
                    "tx" if in_ser => mode = Mode::Name,
                    "cat" if in_ser => mode = Mode::Category,
                    "val" if in_ser => mode = Mode::Value,
                    "spPr" if in_ser => {}
                    "title" => in_title = true,
                    "v" => capture_text = true,
                    "t" if in_title => capture_text = true,
                    "legend" => {
                        chart.legend = Some(true);
                    }
                    "legendPos" => {
                        if let Some(v) = attr(e, b"val") {
                            chart.legend_position = Some(map_legend_pos(&v));
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let n = raw_local_name(e.name().as_ref());
                match n.as_str() {
                    "barDir" => {
                        if let Some(v) = attr(e, b"val") {
                            chart.chart_type =
                                if v == "bar" { "bar" } else { "column" }.to_string();
                        }
                    }
                    "legendPos" => {
                        if let Some(v) = attr(e, b"val") {
                            chart.legend_position = Some(map_legend_pos(&v));
                        }
                    }
                    "srgbClr" if in_ser => {
                        if let Some(v) = attr(e, b"val") {
                            ser.color = Some(v);
                        }
                    }
                    "t" if in_title => {}
                    _ => {}
                }
            }
            Ok(Event::Text(ref t)) if capture_text => {
                let text = t
                    .unescape()
                    .unwrap_or_else(|_| std::str::from_utf8(t.as_ref()).unwrap_or("").into())
                    .to_string();
                text_buf.push_str(&text);
                pending_text = true;
            }
            Ok(Event::End(ref e)) => {
                let n = raw_local_name(e.name().as_ref());
                match n.as_str() {
                    "v" => {
                        if pending_text {
                            let v = std::mem::take(&mut text_buf);
                            enqueue(&mut chart, &mut ser, in_ser, mode, &v);
                            pending_text = false;
                        }
                        capture_text = false;
                    }
                    "t" if in_title => {
                        if pending_text {
                            title_text.push_str(&std::mem::take(&mut text_buf));
                            pending_text = false;
                        }
                        capture_text = false;
                    }
                    "tx" | "cat" | "val" => mode = Mode::None,
                    "title" => in_title = false,
                    "ser" if in_ser => {
                        chart.series.push(std::mem::take(&mut ser));
                        in_ser = false;
                    }
                    "plotArea" => plot_depth = (plot_depth - 1).max(0),
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    let _ = plot_depth;

    if !title_text.trim().is_empty() {
        chart.title = Some(title_text.trim().to_string());
    }
    if chart.chart_type.is_empty() {
        return None;
    }
    Some(chart)
}

/// 按 idx 顺序写入分类/名称/数值
fn enqueue(
    chart: &mut ChartElement,
    ser: &mut ChartSeries,
    in_ser: bool,
    mode: Mode,
    value: &str,
) -> Option<()> {
    match mode {
        Mode::Name if in_ser && ser.name.is_none() => {
            ser.name = Some(value.to_string());
        }
        Mode::Category
            if in_ser
            // 仅在首个系列采集分类
            && chart.series.is_empty() =>
        {
            chart.categories.push(value.to_string());
        }
        Mode::Value if in_ser => {
            if let Ok(v) = value.trim().parse::<f64>() {
                ser.values.push(v);
            }
        }
        _ => {}
    }
    Some(())
}

fn attr(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .map(|a| String::from_utf8_lossy(&a.value).to_string())
}

fn base_type(n: &str) -> &'static str {
    match n {
        "barChart" | "bar3DChart" => "bar",
        "lineChart" | "line3DChart" => "line",
        "pieChart" | "pie3DChart" | "ofPieChart" => "pie",
        "doughnutChart" => "doughnut",
        "areaChart" | "area3DChart" => "area",
        "scatterChart" => "scatter",
        "bubbleChart" => "bubble",
        "stockChart" => "stock",
        "radarChart" => "radar",
        "surfaceChart" | "surface3DChart" => "surface",
        _ => "column",
    }
}

fn map_legend_pos(v: &str) -> String {
    match v {
        "t" => "top",
        "l" => "left",
        "r" => "right",
        "tr" => "topRight",
        _ => "bottom",
    }
    .to_string()
}
