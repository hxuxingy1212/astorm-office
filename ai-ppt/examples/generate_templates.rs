use std::path::Path;

fn generate_template(name: &str, display: &str) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "width": 13.333,
        "height": 7.5,
        "template": name,
        "meta": {
            "title": format!("{} - AI PPT Template", display),
            "author": "AI PPT"
        },
        "slides": [
            {
                "elements": [
                    {
                        "type": "text",
                        "text": display,
                        "position": { "x": 1.5, "y": 1.5, "w": 10.333, "h": 2.0 },
                        "font_size": 54,
                        "bold": true,
                        "color": "accent1",
                        "align": "center",
                        "vert_align": "middle"
                    },
                    {
                        "type": "text",
                        "text": "AI-Powered Presentation Template",
                        "position": { "x": 1.5, "y": 3.5, "w": 10.333, "h": 1.0 },
                        "font_size": 24,
                        "color": "dk2",
                        "align": "center",
                        "vert_align": "middle"
                    },
                    {
                        "type": "text",
                        "text": "json2pptx · Generated from JSON",
                        "position": { "x": 1.5, "y": 4.8, "w": 10.333, "h": 0.8 },
                        "font_size": 16,
                        "color": "accent2",
                        "align": "center",
                        "vert_align": "middle"
                    },
                    {
                        "type": "shape",
                        "shape_type": "rect",
                        "position": { "x": 4.5, "y": 6.2, "w": 4.333, "h": 0.06 },
                        "fill": "accent1",
                        "no_fill": false
                    }
                ]
            },
            {
                "elements": [
                    {
                        "type": "text",
                        "text": "Typography & Colors",
                        "position": { "x": 0.8, "y": 0.4, "w": 11.733, "h": 1.0 },
                        "font_size": 36,
                        "bold": true,
                        "color": "accent1",
                        "align": "left",
                        "vert_align": "middle"
                    },
                    {
                        "type": "shape",
                        "shape_type": "rect",
                        "position": { "x": 0.8, "y": 1.3, "w": 3.0, "h": 0.04 },
                        "fill": "accent2",
                        "no_fill": false
                    },
                    {
                        "type": "text",
                        "text": "Heading 1 — Major font, accent1 color",
                        "position": { "x": 0.8, "y": 1.6, "w": 11.733, "h": 0.8 },
                        "font_size": 28,
                        "bold": true,
                        "color": "dk1",
                        "align": "left",
                        "vert_align": "middle"
                    },
                    {
                        "type": "text",
                        "text": "Heading 2 — Minor font, dk2 color",
                        "position": { "x": 0.8, "y": 2.4, "w": 11.733, "h": 0.7 },
                        "font_size": 22,
                        "color": "dk2",
                        "align": "left",
                        "vert_align": "middle"
                    },
                    {
                        "type": "text",
                        "text": "Body text using the template's font scheme. This demonstrates how the theme colors and fonts flow through to all text elements. Accent colors can be referenced by name like accent1 through accent6.",
                        "position": { "x": 0.8, "y": 3.2, "w": 7.0, "h": 1.2 },
                        "font_size": 16,
                        "color": "dk1",
                        "align": "left",
                        "vert_align": "top"
                    },
                    {
                        "type": "shape",
                        "shape_type": "roundRect",
                        "position": { "x": 8.5, "y": 1.6, "w": 4.0, "h": 1.0 },
                        "fill": "accent1",
                        "text": "Accent 1",
                        "color": "lt1",
                        "font_size": 18,
                        "bold": true,
                        "align": "center",
                        "vert_align": "middle",
                        "shadow": { "blur": 6, "distance": 3, "angle": 45, "opacity": 0.3, "color": "000000" }
                    },
                    {
                        "type": "shape",
                        "shape_type": "roundRect",
                        "position": { "x": 8.5, "y": 2.8, "w": 4.0, "h": 1.0 },
                        "fill": "accent2",
                        "text": "Accent 2",
                        "color": "lt1",
                        "font_size": 18,
                        "bold": true,
                        "align": "center",
                        "vert_align": "middle",
                        "shadow": { "blur": 6, "distance": 3, "angle": 45, "opacity": 0.3, "color": "000000" }
                    },
                    {
                        "type": "shape",
                        "shape_type": "roundRect",
                        "position": { "x": 8.5, "y": 4.0, "w": 4.0, "h": 1.0 },
                        "fill": "accent3",
                        "text": "Accent 3",
                        "color": "lt1",
                        "font_size": 18,
                        "bold": true,
                        "align": "center",
                        "vert_align": "middle",
                        "shadow": { "blur": 6, "distance": 3, "angle": 45, "opacity": 0.3, "color": "000000" }
                    },
                    {
                        "type": "shape",
                        "shape_type": "roundRect",
                        "position": { "x": 8.5, "y": 5.2, "w": 4.0, "h": 1.0 },
                        "fill": "accent4",
                        "text": "Accent 4",
                        "color": "dk1",
                        "font_size": 18,
                        "bold": true,
                        "align": "center",
                        "vert_align": "middle",
                        "shadow": { "blur": 6, "distance": 3, "angle": 45, "opacity": 0.3, "color": "000000" }
                    }
                ]
            },
            {
                "elements": [
                    {
                        "type": "text",
                        "text": "Data Visualization",
                        "position": { "x": 0.8, "y": 0.4, "w": 11.733, "h": 1.0 },
                        "font_size": 36,
                        "bold": true,
                        "color": "accent1",
                        "align": "left",
                        "vert_align": "middle"
                    },
                    {
                        "type": "shape",
                        "shape_type": "rect",
                        "position": { "x": 0.8, "y": 1.3, "w": 3.0, "h": 0.04 },
                        "fill": "accent2",
                        "no_fill": false
                    },
                    {
                        "type": "kpiCard",
                        "position": { "x": 0.8, "y": 1.8, "w": 3.5, "h": 2.0 },
                        "value": "89%",
                        "label": "Customer Satisfaction",
                        "delta": "+12.5%",
                        "bg": "lt2",
                        "accent": "accent1",
                        "rounded": true,
                        "text_color": "dk1"
                    },
                    {
                        "type": "kpiCard",
                        "position": { "x": 4.8, "y": 1.8, "w": 3.5, "h": 2.0 },
                        "value": "$2.4M",
                        "label": "Revenue Growth",
                        "delta": "+8.3%",
                        "bg": "lt2",
                        "accent": "accent2",
                        "rounded": true,
                        "text_color": "dk1"
                    },
                    {
                        "type": "kpiCard",
                        "position": { "x": 8.8, "y": 1.8, "w": 3.5, "h": 2.0 },
                        "value": "1,247",
                        "label": "Active Users",
                        "delta": "+32.1%",
                        "bg": "lt2",
                        "accent": "accent3",
                        "rounded": true,
                        "text_color": "dk1"
                    },
                    {
                        "type": "progressBar",
                        "position": { "x": 0.8, "y": 4.3, "w": 11.733, "h": 0.6 },
                        "value": 75.0,
                        "color": "accent1",
                        "track_color": "lt2",
                        "rounded": true,
                        "show_label": true,
                        "label": "Project Progress",
                        "text_color": "dk1",
                        "font_size": 12
                    },
                    {
                        "type": "progressRing",
                        "position": { "x": 0.8, "y": 5.3, "w": 1.5, "h": 1.5 },
                        "value": 60.0,
                        "color": "accent2",
                        "track_color": "lt2",
                        "thickness": 8.0,
                        "show_label": true,
                        "text_color": "dk1",
                        "font_size": 16
                    },
                    {
                        "type": "barChart",
                        "position": { "x": 3.0, "y": 5.0, "w": 5.0, "h": 2.0 },
                        "data": [45.0, 72.0, 58.0, 90.0, 63.0],
                        "labels": ["Mon", "Tue", "Wed", "Thu", "Fri"],
                        "colors": ["accent1", "accent2", "accent3", "accent4", "accent5"],
                        "max": 100.0,
                        "show_values": true,
                        "axis": true,
                        "label_color": "dk2",
                        "font_size": 10
                    },
                    {
                        "type": "ratingStars",
                        "position": { "x": 8.5, "y": 5.5, "w": 4.0, "h": 0.6 },
                        "rating": 4.5,
                        "max": 5,
                        "color": "accent4",
                        "empty_color": "lt2"
                    }
                ]
            },
            {
                "elements": [
                    {
                        "type": "text",
                        "text": "Content Layouts",
                        "position": { "x": 0.8, "y": 0.4, "w": 11.733, "h": 1.0 },
                        "font_size": 36,
                        "bold": true,
                        "color": "accent1",
                        "align": "left",
                        "vert_align": "middle"
                    },
                    {
                        "type": "shape",
                        "shape_type": "rect",
                        "position": { "x": 0.8, "y": 1.3, "w": 3.0, "h": 0.04 },
                        "fill": "accent2",
                        "no_fill": false
                    },
                    {
                        "type": "table",
                        "position": { "x": 0.8, "y": 1.8, "w": 11.733, "h": 3.0 },
                        "name": "Data Table",
                        "header_row": true,
                        "rows": [
                            [
                                { "text": "Quarter", "font_size": 13, "bold": true, "color": "lt1", "fill": "accent1", "align": "center" },
                                { "text": "Revenue", "font_size": 13, "bold": true, "color": "lt1", "fill": "accent1", "align": "center" },
                                { "text": "Growth", "font_size": 13, "bold": true, "color": "lt1", "fill": "accent1", "align": "center" },
                                { "text": "Status", "font_size": 13, "bold": true, "color": "lt1", "fill": "accent1", "align": "center" }
                            ],
                            [
                                { "text": "Q1 2026", "font_size": 12, "align": "left" },
                                { "text": "$1.2M", "font_size": 12, "align": "right" },
                                { "text": "+8.5%", "font_size": 12, "color": "accent5", "bold": true, "align": "right" },
                                { "text": "On Track", "font_size": 12, "color": "accent5", "bold": true, "align": "center" }
                            ],
                            [
                                { "text": "Q2 2026", "font_size": 12, "align": "left" },
                                { "text": "$1.5M", "font_size": 12, "align": "right" },
                                { "text": "+25.0%", "font_size": 12, "color": "accent5", "bold": true, "align": "right" },
                                { "text": "Exceeded", "font_size": 12, "color": "accent5", "bold": true, "align": "center" }
                            ],
                            [
                                { "text": "Q3 2026", "font_size": 12, "align": "left" },
                                { "text": "$1.1M", "font_size": 12, "align": "right" },
                                { "text": "-6.7%", "font_size": 12, "color": "accent6", "bold": true, "align": "right" },
                                { "text": "At Risk", "font_size": 12, "color": "accent6", "bold": true, "align": "center" }
                            ]
                        ]
                    },
                    {
                        "type": "processFlow",
                        "position": { "x": 0.8, "y": 5.5, "w": 11.733, "h": 0.8 },
                        "steps": ["Ideate", "Design", "Develop", "Test", "Launch"],
                        "colors": ["accent1", "accent2", "accent3", "accent4", "accent5"],
                        "text_color": "lt1",
                        "font_size": 14
                    }
                ]
            },
            {
                "elements": [
                    {
                        "type": "text",
                        "text": "Timeline & Process",
                        "position": { "x": 0.8, "y": 0.4, "w": 11.733, "h": 1.0 },
                        "font_size": 36,
                        "bold": true,
                        "color": "accent1",
                        "align": "left",
                        "vert_align": "middle"
                    },
                    {
                        "type": "shape",
                        "shape_type": "rect",
                        "position": { "x": 0.8, "y": 1.3, "w": 3.0, "h": 0.04 },
                        "fill": "accent2",
                        "no_fill": false
                    },
                    {
                        "type": "timeline",
                        "position": { "x": 0.8, "y": 2.0, "w": 11.733, "h": 1.5 },
                        "items": [
                            "2024 Q1: Research & Planning",
                            "2024 Q2: MVP Development",
                            "2024 Q3: Beta Launch",
                            "2024 Q4: Public Release",
                            "2025 Q1: Scaling"
                        ],
                        "line_color": "accent1",
                        "dot_color": "accent2",
                        "label_color": "dk1"
                    },
                    {
                        "type": "shape",
                        "shape_type": "rect",
                        "position": { "x": 0.8, "y": 4.0, "w": 5.5, "h": 2.5 },
                        "fill": "lt2",
                        "no_fill": false,
                        "shadow": { "blur": 8, "distance": 4, "angle": 45, "opacity": 0.15, "color": "000000" }
                    },
                    {
                        "type": "text",
                        "text": "Key Insight",
                        "position": { "x": 1.3, "y": 4.2, "w": 4.5, "h": 0.6 },
                        "font_size": 20,
                        "bold": true,
                        "color": "accent1",
                        "align": "left",
                        "vert_align": "middle"
                    },
                    {
                        "type": "text",
                        "text": "Early user feedback showed 94% adoption rate within the first month, validating our product-market fit strategy.",
                        "position": { "x": 1.3, "y": 4.9, "w": 4.5, "h": 1.2 },
                        "font_size": 14,
                        "color": "dk2",
                        "align": "left",
                        "vert_align": "top"
                    },
                    {
                        "type": "shape",
                        "shape_type": "rect",
                        "position": { "x": 7.0, "y": 4.0, "w": 5.5, "h": 2.5 },
                        "fill": "accent1",
                        "no_fill": false,
                        "shadow": { "blur": 8, "distance": 4, "angle": 45, "opacity": 0.2, "color": "000000" }
                    },
                    {
                        "type": "text",
                        "text": "Next Steps",
                        "position": { "x": 7.5, "y": 4.2, "w": 4.5, "h": 0.6 },
                        "font_size": 20,
                        "bold": true,
                        "color": "lt1",
                        "align": "left",
                        "vert_align": "middle"
                    },
                    {
                        "type": "text",
                        "text": "• Expand to APAC market\n• Hire 15 new engineers\n• Launch v2.0 in Q2\n• Reach $5M ARR by year end",
                        "position": { "x": 7.5, "y": 4.9, "w": 4.5, "h": 1.4 },
                        "font_size": 14,
                        "color": "lt1",
                        "align": "left",
                        "vert_align": "top"
                    }
                ]
            }
        ]
    });

    let out_dir = Path::new("templates");
    std::fs::create_dir_all(out_dir)?;
    let pptx_path = out_dir.join(format!("template_{}.pptx", name));
    let pres: json2pptx::model::Presentation = serde_json::from_value(json)?;
    let result = json2pptx::generate(&pres, pptx_path.to_str().unwrap())?;
    println!(
        "  ✓ {} -> {} ({} slides)",
        display,
        pptx_path.display(),
        result.slides
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Generating template PPTX files...\n");
    let templates = json2pptx::model::TemplatePreset::all();
    for t in templates {
        generate_template(t.name, t.display)?;
    }
    println!(
        "\nAll {} templates generated in templates/",
        templates.len()
    );
    Ok(())
}
