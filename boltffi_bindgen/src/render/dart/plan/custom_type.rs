#[derive(Debug, Clone)]
pub struct DartCustomType {
    pub name: String,
    pub ty: super::DartType,
    pub doc: Option<String>,
}
