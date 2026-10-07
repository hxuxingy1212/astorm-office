//! 图表部件生成模块
//!
//! 生成 `ppt/charts/chartN.xml`（DrawingML Chart）。数据以缓存形式内联，
//! 不依赖外部嵌入工作簿，PowerPoint / WPS 均可直接打开。

use crate::model::elements::{ChartElement, ChartSeries};
use crate::utils::xml::esc_xml;

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// 生成图表部件 XML
pub fn generate(chart: &ChartElement) -> String {
    // 解析自真实文件的图表：原样回写，保留完整样式
    if let Some(raw) = &chart.raw {
        if !raw.trim().is_empty() {
            return raw.clone();
        }
    }
    let body = match chart.chart_type.as_str() {
        "pie" => pie_like(chart, false),
        "doughnut" => pie_like(chart, true),
        "line" => axis_chart(chart, "lineChart"),
        "area" => axis_chart(chart, "areaChart"),
        "radar" => radar_chart(chart),
        "bar" => bar_chart(chart, "bar"),
        _ => bar_chart(chart, "col"),
    };

    let title = chart
        .title
        .as_deref()
        .filter(|t| !t.is_empty())
        .map(|t| {
            format!(
                "<c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-US\"/><a:t>{}</a:t></a:r></a:p></c:rich></c:tx><c:overlay val=\"0\"/></c:title>",
                esc_xml(t)
            )
        })
        .unwrap_or_default();

    let legend = if chart.legend == Some(false) {
        String::new()
    } else {
        let pos = match chart.legend_position.as_deref() {
            Some("top") => "t",
            Some("left") => "l",
            Some("right") => "r",
            Some("topRight") => "tr",
            _ => "b",
        };
        format!(
            "<c:legend><c:legendPos val=\"{}\"/><c:overlay val=\"0\"/></c:legend>",
            pos
        )
    };

    format!(
        "{}<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" \
         xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
         xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\
         <c:chart>{}<c:plotArea><c:layout/>{}</c:plotArea>{}\
         <c:plotVisOnly val=\"1\"/><c:dispBlanksAs val=\"gap\"/></c:chart></c:chartSpace>",
        XML_DECL, title, body, legend
    )
}

fn bar_chart(chart: &ChartElement, dir: &str) -> String {
    let sers = chart
        .series
        .iter()
        .enumerate()
        .map(|(i, s)| series_xml(i, s, chart, true))
        .collect::<String>();
    format!(
        "<c:barChart><c:barDir val=\"{}\"/><c:grouping val=\"clustered\"/>\
         <c:varyColors val=\"0\"/>{}{}</c:barChart>{}",
        dir,
        sers,
        axes_ids(),
        cat_val_axes()
    )
}

fn axis_chart(chart: &ChartElement, kind: &str) -> String {
    let sers = chart
        .series
        .iter()
        .enumerate()
        .map(|(i, s)| series_xml(i, s, chart, true))
        .collect::<String>();
    let grouping = if kind == "lineChart" {
        "<c:grouping val=\"standard\"/><c:marker val=\"1\"/>"
    } else {
        ""
    };
    format!(
        "<c:{}><c:varyColors val=\"0\"/>{}{}{}</c:{}>{}",
        kind,
        grouping,
        sers,
        axes_ids(),
        kind,
        cat_val_axes()
    )
}

fn radar_chart(chart: &ChartElement) -> String {
    let sers = chart
        .series
        .iter()
        .enumerate()
        .map(|(i, s)| series_xml(i, s, chart, true))
        .collect::<String>();
    format!(
        "<c:radarChart><c:radarStyle val=\"marker\"/><c:varyColors val=\"0\"/>{}{}</c:radarChart>{}",
        sers,
        axes_ids(),
        cat_val_axes()
    )
}

fn pie_like(chart: &ChartElement, doughnut: bool) -> String {
    let sers = chart
        .series
        .iter()
        .enumerate()
        .map(|(i, s)| series_xml(i, s, chart, false))
        .collect::<String>();
    if doughnut {
        format!(
            "<c:doughnutChart><c:varyColors val=\"1\"/><c:holeSize val=\"50\"/>{}</c:doughnutChart>",
            sers
        )
    } else {
        format!("<c:pieChart><c:varyColors val=\"1\"/>{}</c:pieChart>", sers)
    }
}

fn series_xml(idx: usize, s: &ChartSeries, chart: &ChartElement, with_cat: bool) -> String {
    let name = s
        .name
        .clone()
        .unwrap_or_else(|| format!("Series {}", idx + 1));
    let tx = format!(
        "<c:tx><c:strRef><c:f>Sheet1!$A$1</c:f><c:strCache><c:ptCount val=\"1\"/>\
         <c:pt idx=\"0\"><c:v>{}</c:v></c:pt></c:strCache></c:strRef></c:tx>",
        esc_xml(&name)
    );
    let sp_pr = s
        .color
        .as_deref()
        .map(|c| {
            format!(
                "<c:spPr><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill></c:spPr>",
                c.trim_start_matches('#')
            )
        })
        .unwrap_or_default();
    let cat = if with_cat && !chart.categories.is_empty() {
        let pts = chart
            .categories
            .iter()
            .enumerate()
            .map(|(i, c)| format!("<c:pt idx=\"{}\"><c:v>{}</c:v></c:pt>", i, esc_xml(c)))
            .collect::<String>();
        format!(
            "<c:cat><c:strRef><c:f>Sheet1!$A$2</c:f><c:strCache><c:ptCount val=\"{}\"/>{}</c:strCache></c:strRef></c:cat>",
            chart.categories.len(),
            pts
        )
    } else {
        String::new()
    };
    let val_pts = s
        .values
        .iter()
        .enumerate()
        .map(|(i, v)| format!("<c:pt idx=\"{}\"><c:v>{}</c:v></c:pt>", i, fmt_num(*v)))
        .collect::<String>();
    let val = format!(
        "<c:val><c:numRef><c:f>Sheet1!$B$2</c:f><c:numCache><c:formatCode>General</c:formatCode>\
         <c:ptCount val=\"{}\"/>{}</c:numCache></c:numRef></c:val>",
        s.values.len(),
        val_pts
    );
    format!(
        "<c:ser><c:idx val=\"{}\"/><c:order val=\"{}\"/>{}{}{}{}</c:ser>",
        idx, idx, tx, sp_pr, cat, val
    )
}

fn axes_ids() -> &'static str {
    "<c:axId val=\"111111111\"/><c:axId val=\"222222222\"/>"
}

fn cat_val_axes() -> &'static str {
    "<c:catAx><c:axId val=\"111111111\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling>\
     <c:delete val=\"0\"/><c:axPos val=\"b\"/><c:crossAx val=\"222222222\"/></c:catAx>\
     <c:valAx><c:axId val=\"222222222\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling>\
     <c:delete val=\"0\"/><c:axPos val=\"l\"/><c:crossAx val=\"111111111\"/></c:valAx>"
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}
