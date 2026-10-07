//! Tree path grammar, mirroring json2pptx's `/type[index]` style adapted to
//! spreadsheets: `/sheet[1]/cell[A1]`, `/sheet[1]/range[A1:C10]`, etc.

use office_core::CliError;

/// 本模块统一错误类型
type Result<T> = std::result::Result<T, CliError>;

#[derive(Debug, Clone, PartialEq)]
pub enum SheetSel {
    Index(usize),
    Name(String),
    /// 裸 `/sheet`：仅 add sheet 时合法（追加新表）
    Append,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    Cell(String),
    Range(String),
    Row(u32),
    Col(String),
    Chart(usize),
    Image(usize),
    Merge(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub sheet: SheetSel,
    pub target: Option<Target>,
}

/// Parse `/sheet[1]`, `/sheet[Sheet1]/cell[A1]`, ...
pub fn parse(input: &str) -> Result<Path> {
    let s = input.trim();
    if !s.starts_with('/') {
        return Err(CliError::new("路径必须以 / 开头"));
    }
    let segs: Vec<&str> = s
        .trim_start_matches('/')
        .split('/')
        .filter(|x| !x.is_empty())
        .collect();
    if segs.is_empty() {
        return Err(CliError::new("路径为空"));
    }
    let (kind, arg) = match split_seg(segs[0]) {
        Ok(ka) => ka,
        // 裸 `/sheet`：dump 中 add sheet 的写法，等价于追加新表
        Err(_) if segs[0] == "sheet" => {
            if segs.len() > 1 {
                return Err(CliError::new(format!("路径段缺少索引: {}", segs[0])));
            }
            return Ok(Path {
                sheet: SheetSel::Append,
                target: None,
            });
        }
        Err(e) => return Err(e),
    };
    if kind != "sheet" {
        return Err(CliError::new(format!(
            "路径第一段必须是 sheet，实际: {kind}"
        )));
    }
    let sheet = if arg.chars().all(|c| c.is_ascii_digit()) {
        let i: usize = arg
            .parse()
            .map_err(|_| CliError::new(format!("非法 sheet 索引: {arg}")))?;
        if i == 0 {
            return Err(CliError::new("sheet 索引从 1 开始"));
        }
        SheetSel::Index(i)
    } else {
        SheetSel::Name(arg)
    };

    let target = if segs.len() > 1 {
        if segs.len() > 2 {
            return Err(CliError::new("暂不支持多级下钻"));
        }
        let (kind, arg) = split_seg(segs[1])?;
        Some(match kind {
            "cell" => Target::Cell(arg),
            "range" => Target::Range(arg),
            "row" => Target::Row(parse_index(&arg)? as u32),
            "col" => Target::Col(arg),
            "chart" => Target::Chart(parse_index(&arg)?),
            "image" => Target::Image(parse_index(&arg)?),
            "merge" => Target::Merge(parse_index(&arg)?),
            other => return Err(CliError::new(format!("未知元素类型: {other}"))),
        })
    } else {
        None
    };

    Ok(Path { sheet, target })
}

fn split_seg(seg: &str) -> Result<(&str, String)> {
    let open = seg
        .find('[')
        .ok_or_else(|| CliError::new(format!("路径段缺少索引: {seg}")))?;
    if !seg.ends_with(']') {
        return Err(CliError::new(format!("路径段格式错误: {seg}")));
    }
    let kind = &seg[..open];
    let arg = &seg[open + 1..seg.len() - 1];
    if arg.is_empty() {
        return Err(CliError::new(format!("路径段索引为空: {seg}")));
    }
    Ok((kind, arg.to_string()))
}

fn parse_index(arg: &str) -> Result<usize> {
    let i: usize = arg
        .parse()
        .map_err(|_| CliError::new(format!("非法索引: {arg}")))?;
    if i == 0 {
        return Err(CliError::new("索引从 1 开始"));
    }
    Ok(i)
}
