use json2pptx::generate;
use json2pptx::model::elements::*;
use json2pptx::model::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: Some(Meta {
            company: None,
            application: None,
            last_modified_by: None,
            title: Some("Hello json2pptx".to_string()),
            author: Some("Rust".to_string()),
        }),
        template: None,
        theme: Some(Theme {
            colors: Some(std::collections::HashMap::from([
                ("accent1".to_string(), "4472C4".to_string()),
                ("dk1".to_string(), "000000".to_string()),
                ("lt1".to_string(), "FFFFFF".to_string()),
            ])),
            major_font: Some("Calibri Light".to_string()),
            minor_font: Some("Calibri".to_string()),
        }),
        slides: vec![Slide {
            background: Some(serde_json::json!("#1a1a2e")),
            transition: Some(Transition {
                r#type: "fade".to_string(),
                speed: Some("med".to_string()),
                advance_on_click: None,
                advance_after: None,
            }),
            elements: vec![
                Element::Text(TextElement {
                    id: None,
                    placeholder: None,
                    autofit: None,
                    effects: None,
                    vert: None,
                    text: TextContent::Simple("Hello, PowerPoint!".to_string()),
                    position: Position {
                        x: 1.0,
                        y: 2.0,
                        w: 10.0,
                        h: 2.0,
                    },
                    name: None,
                    font_size: Some(48.0),
                    bold: Some(true),
                    italic: None,
                    underline: None,
                    color: Some("FFFFFF".to_string()),
                    font_family: Some("Arial".to_string()),
                    align: Some("center".to_string()),
                    vert_align: Some("middle".to_string()),
                    line_spacing: None,
                    wrap: None,
                    fill: None,
                    fill_alpha: None,
                    line: None,
                    line_alpha: None,
                    shadow: None,
                    animations: None,
                }),
                Element::Shape(ShapeElement {
                    id: None,
                    placeholder: None,
                    autofit: None,
                    effects: None,
                    shape_type: "ellipse".to_string(),
                    position: Position {
                        x: 5.0,
                        y: 5.0,
                        w: 2.0,
                        h: 2.0,
                    },
                    name: None,
                    rotation: None,
                    fill: Some(Fill::Solid("ED7D31".to_string())),
                    no_fill: None,
                    line: Some(Line {
                        color: "FFFFFF".to_string(),
                        width: 2.0,
                    }),
                    shadow: Some(Shadow {
                        blur: 10.0,
                        distance: 5.0,
                        angle: 45.0,
                        opacity: 0.5,
                        color: "000000".to_string(),
                    }),
                    adjust: None,
                    fill_alpha: None,
                    line_alpha: None,
                    text: None,
                    font_size: None,
                    color: None,
                    align: None,
                    vert_align: None,
                    line_spacing: None,
                    animations: None,
                }),
            ],
            notes: None,
        }],
    };

    let result = generate(&pres, "output.pptx")?;
    println!("Generated: {} ({} slides)", result.path, result.slides);
    Ok(())
}
