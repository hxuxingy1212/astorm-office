use json2pptx::generate;
use json2pptx::model::*;
use json2pptx::parse;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let img_path = std::env::current_dir()?.join("dummy.png");
    let dummy_png: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8,
        0xCF, 0xC0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x36, 0x28, 0x19, 0x00, 0x00, 0x00, 0x00,
        0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    std::fs::write(&img_path, dummy_png).ok();
    let img_src = img_path.to_str().unwrap().replace('\\', "/");

    let json_input = serde_json::json!({
        "width": 10.0,
        "height": 5.625,
        "meta": {
            "title": "Comprehensive Test",
            "author": "AI PPT"
        },
        "theme": {
            "colors": {
                "accent1": "4472C4",
                "accent2": "ED7D31",
                "dk1": "000000",
                "lt1": "FFFFFF"
            },
            "major_font": "Calibri Light",
            "minor_font": "Calibri"
        },
        "slides": [
            {
                "elements": [
                    {
                        "type": "text",
                        "text": "Hello World",
                        "position": { "x": 0.5, "y": 0.3, "w": 9.0, "h": 1.0 },
                        "name": "Title Text",
                        "font_size": 44.0, "bold": true, "italic": true, "underline": true,
                        "color": "FF0000", "font_family": "Arial",
                        "align": "center", "vert_align": "middle", "letter_spacing": 2.0, "wrap": false,
                        "fill": "4472C4",
                        "line": { "color": "000000", "width": 1.5 },
                        "shadow": { "blur": 6.0, "distance": 3.0, "angle": 45.0, "opacity": 0.5, "color": "000000" }
                    },
                    {
                        "type": "text",
                        "text": [
                            {
                                "runs": [
                                    { "text": "Bold ", "bold": true, "color": "FF0000", "font_size": 20.0 },
                                    { "text": "Italic ", "italic": true, "color": "00FF00", "font_size": 18.0 },
                                    { "text": "Underline", "underline": true, "color": "0000FF", "font_family": "Times New Roman" }
                                ],
                                "align": "center", "bullet": true
                            },
                            { "text": "Plain text paragraph", "align": "left" }
                        ],
                        "position": { "x": 0.5, "y": 1.5, "w": 9.0, "h": 2.0 },
                        "font_size": 16.0, "font_family": "Arial", "vert_align": "top"
                    },
                    {
                        "type": "text",
                        "text": "Line1\nLine2\nLine3",
                        "position": { "x": 0.5, "y": 3.8, "w": 9.0, "h": 1.0 },
                        "font_size": 14.0
                    },
                    {
                        "type": "text",
                        "text": "<Hello> & 'World' \"Test\" \u{1f4a1}",
                        "position": { "x": 0.5, "y": 5.0, "w": 9.0, "h": 0.6 },
                        "font_size": 12.0
                    }
                ],
                "background": "#1a1a2e",
                "transition": { "type": "fade", "speed": "slow", "advance_on_click": true }
            },
            {
                "elements": [
                    {
                        "type": "shape", "shape_type": "ellipse",
                        "position": { "x": 0.5, "y": 0.5, "w": 4.0, "h": 3.0 },
                        "name": "Ellipse", "rotation": 30.0,
                        "fill": { "stops": [{ "color": "FF0000", "position": 0.0 }, { "color": "0000FF", "position": 1.0 }], "angle": 90.0 },
                        "line": { "color": "FFFFFF", "width": 2.0 },
                        "shadow": { "blur": 8.0, "distance": 4.0, "angle": 135.0, "opacity": 0.6, "color": "000000" },
                        "text": "Gradient Ellipse", "font_size": 18.0, "color": "FFFFFF", "align": "center"
                    },
                    {
                        "type": "shape", "shape_type": "roundRect",
                        "position": { "x": 5.0, "y": 0.5, "w": 4.0, "h": 3.0 },
                        "fill": "ED7D31", "no_fill": false, "text": "Round Rect"
                    },
                    {
                        "type": "shape", "shape_type": "rect",
                        "position": { "x": 0.5, "y": 4.0, "w": 9.0, "h": 1.0 },
                        "fill": "4472C4", "no_fill": true, "text": "No Fill"
                    }
                ]
            },
            {
                "elements": [{
                    "type": "image", "src": img_src,
                    "position": { "x": 0.5, "y": 0.5, "w": 5.0, "h": 4.0 },
                    "name": "Test Image", "rotation": 0.0,
                    "crop": { "left": 10, "top": 20, "right": 30, "bottom": 40 },
                    "hyperlink": "https://example.com",
                    "line": { "color": "FF0000", "width": 3.0 }
                }]
            },
            {
                "elements": [{
                    "type": "table",
                    "position": { "x": 0.5, "y": 0.5, "w": 9.0, "h": 3.0 },
                    "name": "Data Table", "header_row": true,
                    "rows": [
                        [
                            { "text": "Name", "font_size": 14.0, "bold": true, "color": "FFFFFF", "fill": "4472C4", "align": "center" },
                            { "text": "Value", "font_size": 14.0, "bold": true, "color": "FFFFFF", "fill": "4472C4", "align": "center" },
                            { "text": "Status", "font_size": 14.0, "bold": true, "color": "FFFFFF", "fill": "4472C4", "align": "center" }
                        ],
                        [
                            { "text": "Alpha", "font_size": 12.0, "align": "left" },
                            { "text": "42", "font_size": 12.0, "align": "right" },
                            { "text": "OK", "font_size": 12.0, "bold": true, "color": "00AA00", "align": "center" }
                        ],
                        [
                            { "text": "Beta", "align": "left" },
                            { "text": "17", "align": "right", "color": "FF0000" },
                            { "text": "Fail", "color": "FF0000", "bold": true }
                        ]
                    ]
                }]
            },
            {
                "elements": [{
                    "type": "group",
                    "position": { "x": 0.5, "y": 0.5, "w": 9.0, "h": 4.0 },
                    "name": "Group", "rotation": 10.0,
                    "children": [
                        { "type": "text", "text": "Grouped Text", "position": { "x": 0.0, "y": 0.0, "w": 5.0, "h": 1.0 }, "font_size": 24.0, "color": "FF0000" },
                        { "type": "shape", "shape_type": "rect", "position": { "x": 0.0, "y": 1.5, "w": 3.0, "h": 2.0 }, "fill": "00FF00", "line": { "color": "000000", "width": 1.0 } },
                        { "type": "image", "src": img_src, "position": { "x": 4.0, "y": 1.5, "w": 3.0, "h": 2.0 } }
                    ]
                }]
            },
            {
                "elements": [
                    { "type": "progressBar", "position": { "x": 0.5, "y": 0.3, "w": 9.0, "h": 0.6 }, "value": 75.0, "color": "4472C4", "track_color": "E0E0E0", "rounded": true, "show_label": true, "label": "Progress", "text_color": "000000", "font_size": 11.0 },
                    { "type": "progressRing", "position": { "x": 0.5, "y": 1.2, "w": 1.5, "h": 1.5 }, "value": 60.0, "color": "ED7D31", "track_color": "E0E0E0", "thickness": 8.0, "show_label": true, "text_color": "333333" },
                    { "type": "barChart", "position": { "x": 2.5, "y": 1.2, "w": 4.0, "h": 1.5 }, "data": [30.0, 50.0, 80.0, 40.0], "labels": ["Q1", "Q2", "Q3", "Q4"], "colors": ["FF0000", "00FF00", "0000FF", "FFAA00"], "max": 100.0, "show_values": true, "axis": true, "label_color": "333333" },
                    { "type": "kpiCard", "position": { "x": 7.0, "y": 1.2, "w": 2.5, "h": 1.5 }, "value": "98.5%", "label": "Satisfaction", "delta": "+5.2%", "bg": "FFFFFF", "accent": "00AA00", "rounded": true, "text_color": "333333" },
                    { "type": "ratingStars", "position": { "x": 0.5, "y": 3.0, "w": 3.0, "h": 0.6 }, "rating": 4.5, "max": 5, "color": "FFAA00", "empty_color": "E0E0E0" },
                    { "type": "timeline", "position": { "x": 0.5, "y": 3.8, "w": 9.0, "h": 1.0 }, "items": [{"date": "2024-Q1", "title": "Phase 1", "desc": "Research"}, {"date": "2024-Q2", "title": "Phase 2", "desc": "Development"}, {"date": "2024-Q3", "title": "Phase 3", "desc": "Launch"}], "line_color": "4472C4", "dot_color": "ED7D31", "label_color": "333333" },
                    { "type": "processFlow", "position": { "x": 0.5, "y": 5.0, "w": 9.0, "h": 0.6 }, "steps": ["Plan", "Do", "Check", "Act"], "colors": ["4472C4", "ED7D31", "70AD47", "FF0000"], "text_color": "FFFFFF", "font_size": 12.0 }
                ]
            },
            { "elements": [], "background": { "type": "gradient", "stops": [{ "color": "FFE0E0", "position": 0.0 }, { "color": "E0E0FF", "position": 1.0 }], "angle": 45.0 } },
            { "elements": [], "transition": { "type": "wipe", "speed": "fast", "advance_on_click": false, "advance_after": 3000 } },
            { "elements": [], "transition": { "type": "push", "speed": "med" } },
            { "elements": [], "transition": { "type": "cut", "advance_on_click": true } }
        ]
    });

    let out_dir = Path::new(".");
    let json_path = out_dir.join("comprehensive.json");
    let json_str = serde_json::to_string_pretty(&json_input)?;
    std::fs::write(&json_path, &json_str)?;
    println!("JSON input  -> {}", json_path.canonicalize()?.display());

    let pptx_path = out_dir.join("comprehensive.pptx");
    let pres: Presentation = serde_json::from_value(json_input)?;
    generate(&pres, pptx_path.to_str().unwrap())?;
    println!("PPTX output -> {}", pptx_path.canonicalize()?.display());

    let parsed = parse(pptx_path.to_str().unwrap())?;
    let rt_path = out_dir.join("comprehensive_roundtrip.json");
    let rt_json = serde_json::to_string_pretty(&parsed)?;
    std::fs::write(&rt_path, &rt_json)?;
    println!("Roundtrip   -> {}", rt_path.canonicalize()?.display());

    println!(
        "\nDone! {} slides -> {} slides",
        pres.slides.len(),
        parsed.slides.len()
    );
    Ok(())
}
