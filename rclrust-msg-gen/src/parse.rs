use syn::{
    parse::{Parse, ParseStream},
    Attribute,
};

pub struct TypeAttributes {
    pub attrs: Vec<syn::Attribute>,
}

impl Parse for TypeAttributes {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        Ok(Self { attrs })
    }
}
