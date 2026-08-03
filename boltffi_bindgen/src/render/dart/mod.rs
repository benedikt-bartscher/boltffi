mod emit;
mod lower;
pub mod names;
mod plan;
mod templates;
#[cfg(test)]
pub mod test;

pub use emit::*;
pub use lower::DartLowerer;
pub use names::NamingConvention;
pub use plan::*;

pub(crate) fn to_dart_doc(doc: &str) -> String {
    doc.lines().fold(String::new(), |mut acc, s| {
        if !acc.is_empty() {
            acc.push('\n');
        }
        acc.push_str("/// ");
        acc.push_str(s);
        acc
    })
}
