//! 树形路径解析（对应 ai-ppt 的 path.rs）
//!
//! 语法：以 `/` 开头，每段 `类型[索引]`，索引从 1 开始。
//! 对文档：首段为 `part[N]`（别名 `chapter[N]`），其后为块/容器路径。
//!
//! 例：
//!   /part[1]
//!   /part[1]/paragraph[2]
//!   /part[1]/table[1]/row[2]/cell[1]/paragraph[1]
//!   /part[1]/list[1]/item[2]/paragraph[1]

use office_core::CliError;

type Result<T> = std::result::Result<T, CliError>;
use json2docx::model::blocks::Block;
use json2docx::model::Document;

/// 路径段
#[derive(Debug, Clone)]
pub struct Segment {
    pub kind: String,
    pub index: usize,
    /// `[@paraId=XXXX]` 稳定段落 ID 选择器（w14:paraId，插入删除不漂移）
    pub para_id: Option<String>,
}

/// 解析路径字符串
pub fn parse(path: &str) -> Result<Vec<Segment>> {
    if !path.starts_with('/') {
        return Err(CliError::new("路径必须以 / 开头"));
    }
    let mut segs = Vec::new();
    for raw in path.trim_start_matches('/').split('/') {
        if raw.is_empty() {
            continue;
        }
        let open = raw
            .find('[')
            .ok_or_else(|| CliError::new(format!("路径段缺少索引: {raw}")))?;
        let close = raw
            .rfind(']')
            .ok_or_else(|| CliError::new(format!("路径段格式错误: {raw}")))?;
        if close != raw.len() - 1 {
            return Err(CliError::new(format!("路径段格式错误: {raw}")));
        }
        let kind = raw[..open].to_string();
        if kind.is_empty() {
            return Err(CliError::new(format!("路径段缺少类型: {raw}")));
        }
        let idx_str = &raw[open + 1..close];
        if let Some(pid) = idx_str.strip_prefix("@paraId=") {
            if pid.is_empty() {
                return Err(CliError::new(format!("@paraId 不能为空: {raw}")));
            }
            segs.push(Segment {
                kind,
                index: 0,
                para_id: Some(pid.to_string()),
            });
            continue;
        }
        let index: usize = idx_str
            .parse()
            .map_err(|_| format!("索引必须是数字: {raw}"))?;
        if index == 0 {
            return Err(CliError::new("索引从 1 开始"));
        }
        segs.push(Segment {
            kind,
            index,
            para_id: None,
        });
    }
    if segs.is_empty() {
        return Err(CliError::new("路径不能为空"));
    }
    Ok(segs)
}

/// 定位结果
#[derive(Debug, Clone)]
pub struct Located {
    /// 分片下标（0 基）
    pub part: usize,
    /// 块路径（可为空，表示指向整个分片）
    pub block_segs: Vec<Segment>,
}

impl Located {
    /// 是否指向具体元素
    pub fn is_block(&self) -> bool {
        !self.block_segs.is_empty()
    }
}

/// 在文档上解析路径（只做分片级校验；块级在校验时解析）
pub fn resolve(doc: &Document, segs: &[Segment]) -> Result<Located> {
    let first = &segs[0];
    if first.kind != "part" && first.kind != "chapter" {
        return Err(CliError::new(format!(
            "路径第一段必须是 part 或 chapter，实际: {}",
            first.kind
        )));
    }
    let part_idx = first.index - 1;
    if part_idx >= doc.parts.len() {
        return Err(CliError::new(format!(
            "分片索引越界: /{}({})（共 {} 个分片）",
            first.kind,
            first.index,
            doc.parts.len()
        )));
    }
    let block_segs = segs[1..].to_vec();
    if !block_segs.is_empty() {
        // 立即校验块路径可解析
        let _ = resolve_ref(&doc.parts[part_idx].blocks, &block_segs)?;
    }
    Ok(Located {
        part: part_idx,
        block_segs,
    })
}

/// 在块列表中找第 index 个 type 类型的块（1 基）
fn nth_index(blocks: &[Block], kind: &str, index: usize, para_id: Option<&str>) -> Result<usize> {
    if let Some(pid) = para_id {
        return blocks
            .iter()
            .position(|b| b.type_name() == kind && block_para_id(b) == Some(pid))
            .ok_or_else(|| {
                CliError::new(format!(
                    "未找到 @paraId={pid} 的 {kind} 元素（稳定 ID 来自 unpack 的原始文档）"
                ))
            });
    }
    let matches: Vec<usize> = blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| b.type_name() == kind)
        .map(|(i, _)| i)
        .collect();
    if index > matches.len() {
        return Err(CliError::new(format!(
            "未找到第 {} 个 {} 元素（该类型共 {} 个）",
            index,
            kind,
            matches.len()
        ))
        .suggest(if matches.is_empty() {
            format!("当前容器没有 {kind} 元素；用 view 查看现有块类型")
        } else {
            format!("索引范围 1~{}", matches.len())
        }));
    }
    Ok(matches[index - 1])
}

/// 段落块的稳定 ID（@paraId）
fn block_para_id(b: &Block) -> Option<&str> {
    match b {
        Block::Paragraph(p) => p.para_id.as_deref(),
        _ => None,
    }
}

/// 只读解析：返回块引用
pub fn resolve_ref<'a>(blocks: &'a [Block], segs: &[Segment]) -> Result<&'a Block> {
    if segs.is_empty() {
        return Err(CliError::new("路径未指向具体元素"));
    }
    let seg = &segs[0];
    let idx = nth_index(blocks, &seg.kind, seg.index, seg.para_id.as_deref())?;
    if segs.len() == 1 {
        return Ok(&blocks[idx]);
    }
    let rest = &segs[1..];
    match seg.kind.as_str() {
        "table" => {
            let row_seg = rest.first().ok_or("表格路径缺少 row 段")?;
            if row_seg.kind != "row" {
                return Err(CliError::new(format!(
                    "table 下应为 row，实际: {}",
                    row_seg.kind
                )));
            }
            let cell_seg = rest.get(1).ok_or("表格路径缺少 cell 段")?;
            if cell_seg.kind != "cell" {
                return Err(CliError::new(format!(
                    "row 下应为 cell，实际: {}",
                    cell_seg.kind
                )));
            }
            let table = match &blocks[idx] {
                Block::Table(t) => t,
                _ => unreachable!(),
            };
            let ri = row_seg.index - 1;
            if ri >= table.rows.len() {
                return Err(CliError::new(format!(
                    "行越界: row[{}]（共 {} 行）",
                    row_seg.index,
                    table.rows.len()
                )));
            }
            let row = &table.rows[ri];
            let ci = cell_seg.index - 1;
            if ci >= row.cells.len() {
                return Err(CliError::new(format!(
                    "列越界: cell[{}]（共 {} 列）",
                    cell_seg.index,
                    row.cells.len()
                )));
            }
            resolve_ref(&row.cells[ci].blocks, &rest[2..])
        }
        "list" => {
            let item_seg = rest.first().ok_or("列表路径缺少 item 段")?;
            if item_seg.kind != "item" {
                return Err(CliError::new(format!(
                    "list 下应为 item，实际: {}",
                    item_seg.kind
                )));
            }
            let list = match &blocks[idx] {
                Block::List(l) => l,
                _ => unreachable!(),
            };
            let ii = item_seg.index - 1;
            if ii >= list.items.len() {
                return Err(CliError::new(format!(
                    "列表项越界: item[{}]（共 {} 项）",
                    item_seg.index,
                    list.items.len()
                )));
            }
            resolve_ref(&list.items[ii].blocks, &rest[1..])
        }
        other => Err(CliError::new(format!("{other} 不可下钻"))),
    }
}

/// 可变解析：返回块可变引用
pub fn resolve_mut<'a>(blocks: &'a mut [Block], segs: &[Segment]) -> Result<&'a mut Block> {
    if segs.is_empty() {
        return Err(CliError::new("路径未指向具体元素"));
    }
    let seg = segs[0].clone();
    let idx = nth_index(blocks, &seg.kind, seg.index, seg.para_id.as_deref())?;
    if segs.len() == 1 {
        return Ok(&mut blocks[idx]);
    }
    let rest = &segs[1..];
    match seg.kind.as_str() {
        "table" => {
            let row_seg = rest.first().ok_or("表格路径缺少 row 段")?.clone();
            if row_seg.kind != "row" {
                return Err(CliError::new(format!(
                    "table 下应为 row，实际: {}",
                    row_seg.kind
                )));
            }
            let cell_seg = rest.get(1).ok_or("表格路径缺少 cell 段")?.clone();
            if cell_seg.kind != "cell" {
                return Err(CliError::new(format!(
                    "row 下应为 cell，实际: {}",
                    cell_seg.kind
                )));
            }
            let table = match &mut blocks[idx] {
                Block::Table(t) => t,
                _ => unreachable!(),
            };
            let ri = row_seg.index - 1;
            if ri >= table.rows.len() {
                return Err(CliError::new(format!(
                    "行越界: row[{}]（共 {} 行）",
                    row_seg.index,
                    table.rows.len()
                )));
            }
            let row = &mut table.rows[ri];
            let ci = cell_seg.index - 1;
            if ci >= row.cells.len() {
                return Err(CliError::new(format!(
                    "列越界: cell[{}]（共 {} 列）",
                    cell_seg.index,
                    row.cells.len()
                )));
            }
            resolve_mut(&mut row.cells[ci].blocks, &rest[2..])
        }
        "list" => {
            let item_seg = rest.first().ok_or("列表路径缺少 item 段")?.clone();
            if item_seg.kind != "item" {
                return Err(CliError::new(format!(
                    "list 下应为 item，实际: {}",
                    item_seg.kind
                )));
            }
            let list = match &mut blocks[idx] {
                Block::List(l) => l,
                _ => unreachable!(),
            };
            let ii = item_seg.index - 1;
            if ii >= list.items.len() {
                return Err(CliError::new(format!(
                    "列表项越界: item[{}]（共 {} 项）",
                    item_seg.index,
                    list.items.len()
                )));
            }
            resolve_mut(&mut list.items[ii].blocks, &rest[1..])
        }
        other => Err(CliError::new(format!("{other} 不可下钻"))),
    }
}

/// 读取定位块
pub fn block_ref<'a>(doc: &'a Document, loc: &Located) -> Result<&'a Block> {
    resolve_ref(&doc.parts[loc.part].blocks, &loc.block_segs)
}

/// 可变读取定位块
pub fn block_mut<'a>(doc: &'a mut Document, loc: &Located) -> Result<&'a mut Block> {
    resolve_mut(&mut doc.parts[loc.part].blocks, &loc.block_segs)
}

/// 删除定位块并返回它
pub fn remove_block(doc: &mut Document, loc: &Located) -> Result<Block> {
    remove_in(&mut doc.parts[loc.part].blocks, &loc.block_segs)
}

fn remove_in(blocks: &mut Vec<Block>, segs: &[Segment]) -> Result<Block> {
    if segs.is_empty() {
        return Err(CliError::new("路径未指向具体元素"));
    }
    let seg = &segs[0];
    let idx = nth_index(blocks, &seg.kind, seg.index, seg.para_id.as_deref())?;
    if segs.len() == 1 {
        return Ok(blocks.remove(idx));
    }
    let rest = &segs[1..];
    match seg.kind.as_str() {
        "table" => {
            let row_seg = rest.first().ok_or("表格路径缺少 row 段")?.clone();
            let cell_seg = rest.get(1).ok_or("表格路径缺少 cell 段")?.clone();
            if row_seg.kind != "row" || cell_seg.kind != "cell" {
                return Err(CliError::new("表格下应为 row[N]/cell[M]"));
            }
            let table = match &mut blocks[idx] {
                Block::Table(t) => t,
                _ => unreachable!(),
            };
            let (ri, ci) = (row_seg.index - 1, cell_seg.index - 1);
            if ri >= table.rows.len() || ci >= table.rows[ri].cells.len() {
                return Err(CliError::new("表格行列越界"));
            }
            remove_in(&mut table.rows[ri].cells[ci].blocks, &rest[2..])
        }
        "list" => {
            let item_seg = rest.first().ok_or("列表路径缺少 item 段")?.clone();
            if item_seg.kind != "item" {
                return Err(CliError::new("列表下应为 item[N]"));
            }
            let list = match &mut blocks[idx] {
                Block::List(l) => l,
                _ => unreachable!(),
            };
            let ii = item_seg.index - 1;
            if ii >= list.items.len() {
                return Err(CliError::new("列表项越界"));
            }
            remove_in(&mut list.items[ii].blocks, &rest[1..])
        }
        other => Err(CliError::new(format!("{other} 不可删除子元素"))),
    }
}
