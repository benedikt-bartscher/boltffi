use askama::Template as AskamaTemplate;

use crate::{
    core::Result,
    target::kotlin::syntax::{Expression, Identifier, TypeName},
};

#[derive(AskamaTemplate)]
#[template(path = "target/kotlin/equality.kt", escape = "none")]
struct EqualityTemplate<'equality> {
    equality: &'equality StructuralEquality,
}

/// How a generated `equals` compares one property.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Comparison {
    /// `==`, as the data class would.
    Value,
    /// Boxed `equals`, the total order a data class uses for `Float` and `Double`.
    Float { nullable: bool },
    /// `contentEquals`, which also accepts nullable arrays.
    Array,
    /// Element-wise `contentEquals` over a `List` of arrays.
    ArrayList,
}

/// `equals` and `hashCode` for a data class holding arrays, which the
/// generated members would compare by identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuralEquality {
    owner: TypeName,
    equals: Vec<Expression>,
    hash_codes: Vec<Expression>,
}

impl StructuralEquality {
    /// Returns `None` when no property is an array, leaving the data class members in place.
    pub fn new<'property>(
        owner: TypeName,
        properties: impl IntoIterator<Item = (&'property Identifier, Comparison)>,
    ) -> Result<Option<Self>> {
        let properties = properties.into_iter().collect::<Vec<_>>();
        if !properties
            .iter()
            .any(|(_, comparison)| matches!(comparison, Comparison::Array | Comparison::ArrayList))
        {
            return Ok(None);
        }
        let other = Expression::identifier(Identifier::parse("other")?);
        let (equals, hash_codes) = properties
            .into_iter()
            .map(|(name, comparison)| {
                Self::property(
                    Expression::property(Expression::this(), name.clone()),
                    Expression::property(other.clone(), name.clone()),
                    comparison,
                )
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .unzip();
        Ok(Some(Self {
            owner,
            equals,
            hash_codes,
        }))
    }

    pub fn render(&self, prefix: &str) -> Result<String> {
        Ok(EqualityTemplate { equality: self }
            .render()?
            .trim()
            .lines()
            .map(|line| match line.is_empty() {
                true => String::new(),
                false => format!("{prefix}{line}"),
            })
            .collect::<Vec<_>>()
            .join("\n"))
    }

    pub fn owner(&self) -> &TypeName {
        &self.owner
    }

    pub fn equals(&self) -> &[Expression] {
        &self.equals
    }

    pub fn hash_codes(&self) -> &[Expression] {
        &self.hash_codes
    }

    fn property(
        this: Expression,
        other: Expression,
        comparison: Comparison,
    ) -> Result<(Expression, Expression)> {
        let hash_code = Identifier::parse("hashCode")?;
        let content_equals = Identifier::parse("contentEquals")?;
        let content_hash_code = Identifier::parse("contentHashCode")?;
        Ok(match comparison {
            Comparison::Value => (this.clone().equal(other), this.convert(hash_code)),
            Comparison::Float { nullable: false } => (
                Expression::call(
                    this.clone(),
                    Identifier::parse("equals")?,
                    [other].into_iter().collect(),
                ),
                this.convert(hash_code),
            ),
            Comparison::Float { nullable: true } => (
                Expression::safe_call(
                    this.clone(),
                    Identifier::parse("equals")?,
                    [other.clone()].into_iter().collect(),
                )
                .or_else(other.equal(Expression::null()).parenthesized()),
                this.convert(hash_code),
            ),
            Comparison::Array => (
                Expression::call(this.clone(), content_equals, [other].into_iter().collect()),
                this.convert(content_hash_code),
            ),
            Comparison::ArrayList => {
                let size = Identifier::parse("size")?;
                let index = Identifier::parse("index")?;
                let hash = Identifier::parse("hash")?;
                let element = Identifier::parse("element")?;
                let at =
                    |list: &Expression| list.clone().index(Expression::identifier(index.clone()));
                (
                    Expression::property(this.clone(), size.clone())
                        .equal(Expression::property(other.clone(), size))
                        .and(
                            Expression::property(this.clone(), Identifier::parse("indices")?).all(
                                index.clone(),
                                Expression::call(
                                    at(&this),
                                    content_equals,
                                    [at(&other)].into_iter().collect(),
                                ),
                            ),
                        ),
                    this.fold(
                        Expression::integer(1),
                        hash.clone(),
                        element.clone(),
                        Expression::integer(31)
                            .multiply(Expression::identifier(hash))
                            .add(Expression::identifier(element).convert(content_hash_code)),
                    ),
                )
            }
        })
    }
}
