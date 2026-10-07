//! 原生图表 → chartN.xml（DrawingML chart）

use crate::model::blocks::{ChartBlock, ChartSeries};
use crate::utils::xml::{declaration, esc};

const AX_CAT: &str = "111111111";
const AX_VAL: &str = "222222222";

/// 生成 chartN.xml
pub fn chart_xml(chart: &ChartBlock) -> String {
    let mut out = String::new();
    out.push_str(declaration());
    out.push_str(
        "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" \
         xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
         xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">",
    );
    out.push_str("<c:chart>");
    if let Some(title) = &chart.title {
        out.push_str(&format!(
            "<c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>{}</a:t></a:r></a:p></c:rich></c:tx><c:overlay val=\"0\"/></c:title>",
            esc(title)
        ));
        out.push_str("<c:autoTitleDeleted val=\"0\"/>");
    } else {
        out.push_str("<c:autoTitleDeleted val=\"1\"/>");
    }
    out.push_str("<c:plotArea><c:layout/>");

    let kind = chart.chart_type.to_ascii_lowercase();
    match kind.as_str() {
        "pie" | "doughnut" => out.push_str(&pie_like(chart, &kind)),
        "line" => out.push_str(&line_chart(chart)),
        "area" => out.push_str(&area_chart(chart)),
        "bar" => out.push_str(&bar_chart(chart, "bar")),
        _ => out.push_str(&bar_chart(chart, "col")),
    }

    if !matches!(kind.as_str(), "pie" | "doughnut") {
        out.push_str(&axes(chart));
    }
    out.push_str("</c:plotArea>");
    let legend = chart.legend.as_deref().unwrap_or("b");
    if legend != "none" {
        out.push_str(&format!(
            "<c:legend><c:legendPos val=\"{}\" /><c:overlay val=\"0\"/></c:legend>",
            esc(legend)
        ));
    }
    out.push_str("<c:plotVisOnly val=\"1\"/></c:chart></c:chartSpace>");
    out
}

fn series_title(n: usize, name: &str) -> String {
    if name.is_empty() {
        format!("系列{}", n + 1)
    } else {
        name.to_string()
    }
}

/// 字符串字面量缓存（类别）
fn str_lit(values: &[String]) -> String {
    let mut s = format!("<c:strLit><c:ptCount val=\"{}\"/>", values.len());
    for (i, v) in values.iter().enumerate() {
        s.push_str(&format!("<c:pt idx=\"{i}\"><c:v>{}</c:v></c:pt>", esc(v)));
    }
    s.push_str("</c:strLit>");
    s
}

/// 数值字面量缓存
fn num_lit(values: &[f64]) -> String {
    let mut s = format!("<c:numLit><c:ptCount val=\"{}\"/>", values.len());
    for (i, v) in values.iter().enumerate() {
        s.push_str(&format!("<c:pt idx=\"{i}\"><c:v>{v}</c:v></c:pt>"));
    }
    s.push_str("</c:numLit>");
    s
}

fn ser_common(idx: usize, ser: &ChartSeries, cats: &[String], data_labels: bool) -> String {
    let title = series_title(idx, &ser.name);
    let sp_pr = match &ser.color {
        Some(c) => format!(
            "<c:spPr><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill></c:spPr>",
            crate::utils::xml::color_hex(c)
        ),
        None => "<c:spPr/>".to_string(),
    };
    let mut s = format!(
        "<c:idx val=\"{idx}\"/><c:order val=\"{idx}\"/>\
         <c:tx><c:v>{}</c:v></c:tx>{sp_pr}",
        esc(&title)
    );
    if !cats.is_empty() {
        s.push_str(&format!("<c:cat>{}</c:cat>", str_lit(cats)));
    }
    s.push_str(&format!("<c:val>{}</c:val>", num_lit(&ser.values)));
    if data_labels {
        s.push_str("<c:dLbls><c:showVal val=\"1\"/><c:showCatName val=\"0\"/><c:showSerName val=\"0\"/></c:dLbls>");
    }
    s
}

fn bar_chart(chart: &ChartBlock, dir: &str) -> String {
    let g = chart
        .grouping
        .clone()
        .unwrap_or_else(|| "clustered".to_string());
    let mut s = format!(
        "<c:barChart><c:barDir val=\"{dir}\"/><c:grouping val=\"{g}\"/><c:varyColors val=\"0\"/>"
    );
    if g != "clustered" {
        s.push_str("<c:overlap val=\"100\"/>");
    }
    for (i, ser) in chart.series.iter().enumerate() {
        s.push_str(&format!(
            "<c:ser>{}</c:ser>",
            ser_common(i, ser, &chart.categories, chart.data_labels == Some(true))
        ));
    }
    s.push_str(&format!(
        "<c:axId val=\"{AX_CAT}\"/><c:axId val=\"{AX_VAL}\"/></c:barChart>"
    ));
    s
}

fn line_chart(chart: &ChartBlock) -> String {
    let g = chart
        .grouping
        .clone()
        .unwrap_or_else(|| "standard".to_string());
    let mut s = format!("<c:lineChart><c:grouping val=\"{g}\"/><c:varyColors val=\"0\"/>");
    for (i, ser) in chart.series.iter().enumerate() {
        s.push_str(&format!(
            "<c:ser><c:marker><c:symbol val=\"none\"/></c:marker>{}</c:ser>",
            ser_common(i, ser, &chart.categories, chart.data_labels == Some(true))
        ));
    }
    s.push_str(&format!(
        "<c:axId val=\"{AX_CAT}\"/><c:axId val=\"{AX_VAL}\"/></c:lineChart>"
    ));
    s
}

fn area_chart(chart: &ChartBlock) -> String {
    let g = chart
        .grouping
        .clone()
        .unwrap_or_else(|| "standard".to_string());
    let mut s = format!("<c:areaChart><c:grouping val=\"{g}\"/><c:varyColors val=\"0\"/>");
    for (i, ser) in chart.series.iter().enumerate() {
        s.push_str(&format!(
            "<c:ser>{}</c:ser>",
            ser_common(i, ser, &chart.categories, chart.data_labels == Some(true))
        ));
    }
    s.push_str(&format!(
        "<c:axId val=\"{AX_CAT}\"/><c:axId val=\"{AX_VAL}\"/></c:areaChart>"
    ));
    s
}

fn pie_like(chart: &ChartBlock, kind: &str) -> String {
    let mut s = if kind == "doughnut" {
        String::from("<c:doughnutChart><c:varyColors val=\"1\"/>")
    } else {
        String::from("<c:pieChart><c:varyColors val=\"1\"/>")
    };
    for (i, ser) in chart.series.iter().enumerate() {
        s.push_str(&format!(
            "<c:ser>{}</c:ser>",
            ser_common(i, ser, &chart.categories, chart.data_labels == Some(true))
        ));
    }
    if kind == "doughnut" {
        s.push_str("<c:holeSize val=\"50\"/></c:doughnutChart>");
    } else {
        s.push_str("</c:pieChart>");
    }
    s
}

fn ax_title(text: &str) -> String {
    format!(
        "<c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>{}</a:t></a:r></a:p></c:rich></c:tx><c:overlay val=\"0\"/></c:title>",
        esc(text)
    )
}

fn axes(chart: &ChartBlock) -> String {
    let mut scaling = String::from("<c:orientation val=\"minMax\"/>");
    if let Some(m) = chart.y_min {
        scaling.push_str(&format!("<c:min val=\"{m}\"/>"));
    }
    if let Some(m) = chart.y_max {
        scaling.push_str(&format!("<c:max val=\"{m}\"/>"));
    }
    let cat_title = chart.x_title.as_deref().map(ax_title).unwrap_or_default();
    let val_title = chart.y_title.as_deref().map(ax_title).unwrap_or_default();
    format!(
        concat!(
            "<c:catAx><c:axId val=\"{cat}\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling>",
            "<c:delete val=\"0\"/><c:axPos val=\"b\"/>{cat_title}<c:crossAx val=\"{val}\"/>",
            "<c:tickLblPos val=\"nextTo\"/><c:crosses val=\"autoZero\"/><c:auto val=\"1\"/>",
            "<c:lblAlgn val=\"ctr\"/><c:lblOffset val=\"100\"/></c:catAx>",
            "<c:valAx><c:axId val=\"{val}\"/><c:scaling>{scaling}</c:scaling>",
            "<c:delete val=\"0\"/><c:axPos val=\"l\"/><c:majorGridlines/>{val_title}<c:crossAx val=\"{cat}\"/>",
            "<c:tickLblPos val=\"nextTo\"/><c:crosses val=\"autoZero\"/><c:crossBetween val=\"between\"/>",
            "<c:majorTickMark val=\"none\"/></c:valAx>"
        ),
        cat = AX_CAT,
        val = AX_VAL,
        cat_title = cat_title,
        val_title = val_title,
        scaling = scaling
    )
}
