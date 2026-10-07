pub mod cell;
pub mod sheet;
pub mod style;
pub mod workbook;

pub use cell::{Cell, CellStyleRef, Run};
pub use sheet::{
    Chart, Column, ConditionalFormat, DataValidation, FilterColumn, Image, PrintSettings, Row,
    Shape, Sheet, Size, Table,
};
pub use style::{Alignment, Border, Font, StyleDef};
pub use workbook::{Meta, NamedRange, Workbook};

pub(crate) fn is_false(b: &bool) -> bool {
    !*b
}
