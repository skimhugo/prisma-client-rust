use convert_case::{Case, Casing};
use quote::{__private::TokenStream, format_ident, quote};
use syn::Ident;

use crate::generator::{
    ast::{dmmf::Document, Model, Field},
    Root, GraphQLType,
};

struct Outputs {
    outputs: Vec<String>
}

impl Outputs {
    pub fn new(model: &Model) -> Self {
        Self {
            outputs: model
                .fields
                .iter()
                .filter(|f| f.kind.include_in_struct())
                .map(|f| f.name.to_string())
                .collect()
        }
    }

    pub fn quote(&self) -> TokenStream {
        let Self { outputs } = self;

        quote! {
            pub fn _outputs() -> Vec<Selection> {
                [#(#outputs),*]
                    .into_iter()
                    .map(|o| {
                        let builder = Selection::builder(o);
                        builder.build()
                    })
                    .collect()
            }
        }
    }
}

struct WithParams {
    variants: Vec<TokenStream>,
    match_arms: Vec<TokenStream>,
    from_args: Vec<TokenStream>
}

impl WithParams {
    pub fn new() -> Self {
        Self {
            variants: vec![],
            match_arms: vec![],
            from_args: vec![]
        }
    }
    
    fn with_fn(module: &Ident) -> TokenStream {
        quote! {
            pub fn with(mut self, params: impl Into<#module::WithParam>) -> Self {
                self.args = self.args.with(params.into());
                self
            }
        }
    }
    
    fn add_single_variant(&mut self, field_name: &str, model_module: &Ident, variant_name: &Ident) {
        self.variants.push(quote!(#variant_name(super::#model_module::UniqueArgs)));
        self.match_arms.push(quote! {
            Self::#variant_name(args) => {
                let mut selections = super::#model_module::_outputs();
                selections.extend(args.with_params.into_iter().map(Into::<Selection>::into));
                
                let mut builder = Selection::builder(#field_name);
                builder.nested_selections(selections);
                builder.build()
            }
        });
    }
    
    fn add_many_variant(&mut self, field_name: &str, model_module: &Ident, variant_name: &Ident) {
        self.variants.push(quote!(#variant_name(super::#model_module::ManyArgs)));
        self.match_arms.push(quote! {
            Self::#variant_name(args) => {
                let (
                    arguments,
                    mut nested_selections
                 ) = args.to_graphql();
                nested_selections.extend(super::#model_module::_outputs());
                
                let mut builder = Selection::builder(#field_name);
                builder.nested_selections(nested_selections)
                    .set_arguments(arguments);
                builder.build()
            }
        });
    }

    pub fn quote(&self) -> TokenStream {
        let Self {
            variants,
            match_arms,
            from_args,
            ..
        } = self;

        quote! {
            pub enum WithParam {
                #(#variants),*
            }
            
            impl Into<Selection> for WithParam {
                fn into(self) -> Selection {
                    match self {
                        #(#match_arms),*
                    }
                }
            }
            
            #(#from_args)*
        }
    }
}

struct SetParams {
    variants: Vec<TokenStream>,
    match_arms: Vec<TokenStream>,
}

impl SetParams {
    pub fn new() -> Self {
        Self {
            variants: vec![],
            match_arms: vec![],
        }
    }

    fn add_variant(&mut self, variant: TokenStream, match_arm: TokenStream) {
        self.variants.push(variant);
        self.match_arms.push(match_arm);
    }

    pub fn quote(&self) -> TokenStream {
        let Self {
            variants,
            match_arms,
            ..
        } = self;

        quote! {
            pub enum SetParam {
                #(#variants),*
            }
            
            impl Into<(String, PrismaValue)> for SetParam {
                fn into(self) -> (String, PrismaValue) {
                    match self {
                        #(#match_arms),*
                    }
                }
            }
        }
    } 

    pub fn field_link_variant(field_name: &str) -> Ident {
        format_ident!("Link{}", field_name.to_case(Case::Pascal))
    }

    pub fn field_unlink_variant(field_name: &str) -> Ident {
        format_ident!("Unlink{}", field_name.to_case(Case::Pascal))
    }

    pub fn field_set_variant(field_name: &str) -> Ident {
        format_ident!("Set{}", field_name.to_case(Case::Pascal))
    }
}

struct OrderByParams {
    variants: Vec<TokenStream>,
    match_arms: Vec<TokenStream>,
}

impl OrderByParams {
    pub fn new() -> Self {
        Self {
            variants: vec![],
            match_arms: vec![],
        }
    }
    
    fn order_by_fn(module: &Ident) -> TokenStream {
        quote! {
            pub fn order_by(mut self, param: #module::OrderByParam) -> Self {
                self.args = self.args.order_by(param);
                self
            }
        }
    }

    fn add_variant(&mut self, field_name: &str, variant_name: &Ident) {
        self.variants.push(quote!(#variant_name(Direction)));
        self.match_arms.push(quote! {
            Self::#variant_name(direction) => (
                #field_name.to_string(), 
                PrismaValue::String(direction.to_string())
            ) 
        });
    }

    pub fn quote(&self) -> TokenStream {
        let Self {
            variants,
            match_arms,
            ..
        } = self;

        quote! {
            pub enum OrderByParam {
                #(#variants),*
            }
            
            impl Into<(String, PrismaValue)> for OrderByParam {
                fn into(self) -> (String, PrismaValue) {
                    match self {
                        #(#match_arms),*
                    }
                }
            }
        }
    }
}

struct PaginationParams {
    cursor_variants: Vec<TokenStream>,
    cursor_match_arms: Vec<TokenStream>,
}

impl PaginationParams {
    pub fn new() -> Self {
        Self {
            cursor_variants: vec![],
            cursor_match_arms: vec![],
        }
    }
    
    pub fn pagination_fns(module: &Ident) -> TokenStream {
        quote! {
            pub fn skip(mut self, value: i64) -> Self {
                self.args = self.args.skip(value);
                self
            }

            pub fn take(mut self, value: i64) -> Self {
                self.args = self.args.take(value);
                self
            }

            pub fn cursor(mut self, value: impl Into<#module::Cursor>) -> Self {
                self.args = self.args.cursor(value.into());
                self
            }
        }
    }

    pub fn add_variant(&mut self, field_name: &str, variant_name: &Ident, cursor_type: &GraphQLType) {
        let rust_type = cursor_type.tokens();
        self.cursor_variants.push(quote!(#variant_name(#rust_type)));
        
        let prisma_value = cursor_type.to_prisma_value(&format_ident!("cursor"));
        self.cursor_match_arms.push(quote! {
            Self::#variant_name(cursor) => (
                #field_name.to_string(),
                #prisma_value.into()
            )
        });
    }

    pub fn quote(&self) -> TokenStream {
        let Self {
            cursor_variants,
            cursor_match_arms,
            ..
        } = self;

        quote! {
            pub enum Cursor {
                #(#cursor_variants),*
            }
            
            impl Into<(String, PrismaValue)> for Cursor {
                fn into(self) -> (String, PrismaValue) {
                    match self {
                        #(#cursor_match_arms),*
                    }
                }
            }
        }
    }
}

struct FieldQueryModule {
    name: Ident,
    methods: Vec<TokenStream>,
    structs: Vec<TokenStream>
}

impl FieldQueryModule {
    pub fn new(field: &Field) -> Self {
        Self {
            name: format_ident!("{}", field.name.to_case(Case::Snake)),
            methods: vec![],
            structs: vec![],
        }
    }
    
    pub fn add_method(&mut self, method: TokenStream) {
        self.methods.push(method);
    }
    
    pub fn add_struct(&mut self, struct_: TokenStream) {
        self.structs.push(struct_);
    }
    
    pub fn quote(&self) -> TokenStream {
        let Self {
            name,
            methods,
            structs,
        } = self;
        
        quote! {
            pub mod #name {
                use super::super::*;
                use super::{WhereParam, UniqueWhereParam, OrderByParam, Cursor, WithParam, SetParam};
                
                #(#methods)*
                
                #(#structs)*
            }
        }
    }
}

struct ModelQueryModules {
    field_modules: Vec<FieldQueryModule>,
    compound_field_accessors: Vec<TokenStream>
}

impl ModelQueryModules {
    pub fn new() -> Self {
        Self {
            field_modules: vec![],
            compound_field_accessors: vec![]
        }
    }
    
    pub fn add_field_module(&mut self, field_module: FieldQueryModule) {
        self.field_modules.push(field_module);
    }
    
    pub fn add_compound_field(&mut self, compound_field_accessor: TokenStream) {
        self.compound_field_accessors.push(compound_field_accessor);
    }
    
    pub fn quote(&self) -> TokenStream {
        let Self {
            
            field_modules,
            compound_field_accessors
        } = self;
        
        let field_modules = field_modules.iter().map(|field| field.quote()).collect::<Vec<_>>();
        
        quote! {
            #(#field_modules)*
            
            #(#compound_field_accessors)*
        }
    }
}

struct WhereParams {
    pub variants: Vec<TokenStream>,
    pub to_query_value: Vec<TokenStream>,
    pub unique_variants: Vec<TokenStream>,
    pub from_unique_match_arms: Vec<TokenStream>,
    pub from_optional_uniques: Vec<TokenStream>
}

impl WhereParams {
    pub fn new() -> Self {
        Self {
            variants: vec![],
            to_query_value: vec![],
            unique_variants: vec![],
            from_unique_match_arms: vec![],
            from_optional_uniques: vec![]
        }
    }

    pub fn add_variant(&mut self, variant: TokenStream, match_arm: TokenStream) {
        self.variants.push(variant);
        self.to_query_value.push(match_arm);
    }

    pub fn add_unique_variant(
        &mut self,
        variant: TokenStream,
        match_arm: TokenStream,
        from_unique_match_arm: TokenStream,
        unique_variant: TokenStream
    ) {
        self.add_variant(variant, match_arm);
        self.unique_variants.push(unique_variant);
        self.from_unique_match_arms.push(from_unique_match_arm);
    }
    
    pub fn add_optional_unique_variant(
        &mut self,
        variant: TokenStream,
        match_arm: TokenStream,
        from_unique_match_arm: TokenStream,
        unique_variant: TokenStream,
        arg_type: &TokenStream,
        variant_name: &syn::Ident,
        struct_name: TokenStream
    ) {
        self.add_unique_variant(variant, match_arm, from_unique_match_arm, unique_variant);
        
        self.from_optional_uniques.push(quote!{
            impl prisma_client_rust::traits::FromOptionalUniqueArg<#struct_name> for WhereParam {
                type Arg = Option<#arg_type>;
                
                fn from_arg(arg: Self::Arg) -> Self where Self: Sized {
                    Self::#variant_name(arg)
                }
            }
            
            impl prisma_client_rust::traits::FromOptionalUniqueArg<#struct_name> for UniqueWhereParam {
                type Arg = #arg_type;
                
                fn from_arg(arg: Self::Arg) -> Self where Self: Sized {
                    Self::#variant_name(arg)
                }
            }
        });
    }

    pub fn quote(&self) -> TokenStream {
        let Self {
            variants,
            to_query_value,
            unique_variants,
            from_unique_match_arms,
            from_optional_uniques
        } = self;

        quote! {
            pub enum WhereParam {
                #(#variants),*
            }
            
            impl Into<SerializedWhere> for WhereParam {
                fn into(self) -> SerializedWhere {
                    match self {
                        #(#to_query_value),*
                    }
                }
            }

            pub enum UniqueWhereParam {
                #(#unique_variants),*
            }

            impl From<UniqueWhereParam> for WhereParam {
                fn from(value: UniqueWhereParam) -> Self {
                    match value {
                        #(#from_unique_match_arms),*
                    }
                }
            }
            
            #(#from_optional_uniques)*

            impl From<Operator<Self>> for WhereParam {
                fn from(op: Operator<Self>) -> Self {
                    match op {
                        Operator::Not(value) => Self::Not(value),
                        Operator::And(value) => Self::And(value),
                        Operator::Or(value) => Self::Or(value),
                    }
                }
            }
        }
    }
}

struct DataStruct {
    fields: Vec<TokenStream>,
}

impl DataStruct {
    pub fn new() -> Self {
        Self {
            fields: vec![],
        }
    }

    pub fn add_field(&mut self, field: TokenStream) {
        self.fields.push(field);
    }

    pub fn quote(&self) -> TokenStream {
        let Self {
            fields,
        } = self;

        quote! {
            #[derive(Debug, Clone, Serialize, Deserialize)]
            pub struct Data {
                #(#fields),*
            }
        }
    }
}

struct Actions {
    pub create_args: Vec<TokenStream>,
    pub create_args_tuple_types: Vec<TokenStream>,
    pub create_args_destructured: Vec<TokenStream>,
    pub create_args_params_pushes: Vec<TokenStream>,
}

impl Actions {
    pub fn new() -> Self {
        Self {
            create_args: vec![],
            create_args_tuple_types: vec![],
            create_args_destructured: vec![],
            create_args_params_pushes: vec![],
        }
    }

    pub fn push_required_arg(&mut self, field_name: &Ident, variant_type: Ident) {
        self.create_args.push(quote!(#field_name: #field_name::#variant_type,));
        self.create_args_tuple_types.push(quote!(#field_name::#variant_type,));
        self.create_args_destructured.push(quote!(#field_name,));
        self.create_args_params_pushes.push(quote!(_params.push(#field_name.into());));
    }
}

pub fn generate(root: &Root) -> Vec<TokenStream> {
    root.ast.as_ref().unwrap().models.iter().map(|model| {
        let model_outputs = Outputs::new(&model);
        let mut model_data_struct = DataStruct::new();
        let mut model_order_by_params = OrderByParams::new();
        let mut model_pagination_params = PaginationParams::new();
        let mut model_with_params = WithParams::new();
        let mut model_query_module = ModelQueryModules::new();
        let mut model_set_params = SetParams::new();
        let mut model_where_params = WhereParams::new();
        let mut model_actions = Actions::new();
        
        let model_name_string = &model.name;
        let model_name_snake = format_ident!("{}", model.name.to_case(Case::Snake));
 
        for op in Document::operators() {
            let variant_name = format_ident!("{}", op.name.to_case(Case::Pascal));
            let op_action = &op.action;

            model_where_params.add_variant(
                quote!(#variant_name(Vec<WhereParam>)),
                quote! {
                    Self::#variant_name(value) => (
                        #op_action.to_string(),
                        SerializedWhereValue::List(
                            value
                                .into_iter()
                                .map(|v| PrismaValue::Object(transform_equals(vec![v])))
                                .collect(),
                        ),
                    )
                },
            );
        }

        for unique in &model.indexes {
            let variant_name_string = unique.fields.iter().map(|f| f.to_case(Case::Pascal)).collect::<String>();
            let variant_name = format_ident!("{}Equals", &variant_name_string);
            let accessor_name = format_ident!("{}", &variant_name_string.to_case(Case::Snake));
            
            let mut variant_data_as_types = vec![];
            let mut variant_data_as_args = vec![];
            let mut variant_data_as_destructured = vec![];
            let mut variant_data_as_query_values = vec![];
            let variant_data_names = unique.fields.iter().map(ToString::to_string).collect::<Vec<_>>();
            
            for field in &unique.fields {
                let model_field = model.fields.iter().find(|mf| &mf.name == field).unwrap();
                let field_type = model_field.field_type.tokens();
                
                let field_name_snake = format_ident!("{}", field.to_case(Case::Snake));
                
                let field_type = match model_field.is_list {
                    true => quote!(Vec<#field_type>),
                    false => quote!(#field_type),
                };
                
                variant_data_as_args.push(quote!(#field_name_snake: #field_type));
                variant_data_as_types.push(field_type);
                variant_data_as_destructured.push(quote!(#field_name_snake));
                variant_data_as_query_values.push(model_field.field_type.to_query_value(&field_name_snake, model_field.is_list));
            }

            let field_name_string = unique.fields.join("_");

            model_query_module.add_compound_field(
                quote! {
                    pub fn #accessor_name<T: From<UniqueWhereParam>>(#(#variant_data_as_args),*) -> T {
                        UniqueWhereParam::#variant_name(#(#variant_data_as_destructured),*).into()
                    }
                }
            );

            model_where_params.add_unique_variant(
                quote!(#variant_name(#(#variant_data_as_types),*)),
                quote! {
                    Self::#variant_name(#(#variant_data_as_destructured),*) => (
                        #field_name_string.to_string(),
                        SerializedWhereValue::Object(vec![#((#variant_data_names.to_string(), #variant_data_as_query_values)),*])
                    )
                },
                quote! {
                    UniqueWhereParam::#variant_name(#(#variant_data_as_destructured),*) => Self::#variant_name(#(#variant_data_as_destructured),*)
                },
                quote!(#variant_name(#(#variant_data_as_types),*)),
            );
        }

        for field in &model.fields {
            let mut field_query_module =
                FieldQueryModule::new(&field);

            let field_string = &field.name;
            let field_snake = format_ident!("{}", field.name.to_case(Case::Snake));
            let field_pascal = format_ident!("{}", field.name.to_case(Case::Pascal));
            let field_type_tokens_string = field.field_type.value();
            let field_type = field.field_type.tokens();
            
            if field.kind.is_relation() {
                let link_variant = SetParams::field_link_variant(&field.name);
                let unlink_variant = SetParams::field_unlink_variant(&field.name);

                let relation_type_snake = format_ident!("{}", field_type_tokens_string.to_case(Case::Snake));

                // Relation methods eg. Every, Some, Is
                for method in field.relation_methods() {
                    let method_action_string = &method.action;
                    let variant_name =
                        format_ident!("{}{}", &field_pascal, method.name.to_case(Case::Pascal));
                    let method_name_snake = format_ident!("{}", method.name.to_case(Case::Snake));

                    model_where_params.add_variant(
                        quote!(#variant_name(Vec<super::#relation_type_snake::WhereParam>)),
                        quote! {
                            Self::#variant_name(value) => (
                                #field_string.to_string(),
                                SerializedWhereValue::Object(vec![(
                                    #method_action_string.to_string(),
                                    PrismaValue::Object(transform_equals(value)),
                                )])
                            )
                        },
                    );

                    field_query_module.add_method(quote! {
                        pub fn #method_name_snake(value: Vec<#relation_type_snake::WhereParam>) -> WhereParam {
                            WhereParam::#variant_name(value)
                        }
                    });
                }

                let with_fn = WithParams::with_fn(&relation_type_snake);

                // Relation actions eg. Fetch, Link, Unlink
                if field.is_list {
                    let order_by_fn = OrderByParams::order_by_fn(&relation_type_snake);
                    let pagination_fns = PaginationParams::pagination_fns(&relation_type_snake);
                    
                    field_query_module.add_method(quote! {
                        pub struct Fetch {
                            args: #relation_type_snake::ManyArgs
                        }
                        
                        impl Fetch {
                            #with_fn
                            
                            #order_by_fn
                            
                            #pagination_fns
                        }
                        
                        impl From<Fetch> for WithParam {
                            fn from(fetch: Fetch) -> Self {
                                WithParam::#field_pascal(fetch.args)
                            }
                        }
                        
                        pub fn fetch(params: Vec<#relation_type_snake::WhereParam>) -> Fetch {
                            Fetch {
                                args: #relation_type_snake::ManyArgs::new(params)
                            }
                        }

                        pub fn link<T: From<Link>>(params: Vec<#relation_type_snake::UniqueWhereParam>) -> T {
                            Link(params).into()
                        }

                        pub fn unlink(params: Vec<#relation_type_snake::UniqueWhereParam>) -> SetParam {
                            SetParam::#unlink_variant(params)
                        }
                    });

                    field_query_module.add_struct(quote! {
                        pub struct Link(Vec<#relation_type_snake::UniqueWhereParam>);

                        impl From<Link> for SetParam {
                            fn from(value: Link) -> Self {
                                Self::#link_variant(value.0)
                            }
                        }
                    });

                    model_set_params.add_variant(
                        quote!(#link_variant(Vec<super::#relation_type_snake::UniqueWhereParam>)),
                        quote! {
                            SetParam::#link_variant(where_params) => (
                                #field_string.to_string(),
                                PrismaValue::Object(
                                    vec![(
                                        "connect".to_string(),
                                        PrismaValue::Object(
                                            transform_equals(
                                                where_params
                                                    .into_iter()
                                                    .map(Into::<super::#relation_type_snake::WhereParam>::into)
                                                    .collect()
                                            )
                                        )
                                    )]
                                )
                            )
                        }
                    );

                    model_set_params.add_variant(
                        quote!(#unlink_variant(Vec<super::#relation_type_snake::UniqueWhereParam>)),
                        quote! {
                            SetParam::#unlink_variant(where_params) => (
                                #field_string.to_string(),
                                PrismaValue::Object(
                                    vec![(
                                        "disconnect".to_string(),
                                        PrismaValue::Object(
                                            transform_equals( 
                                                where_params
                                                    .into_iter()
                                                    .map(Into::<super::#relation_type_snake::WhereParam>::into)
                                                    .collect()
                                            )
                                        )
                                    )]
                                )
                            )
                        },
                    );
                    
                    model_with_params.add_many_variant(
                        field_string,
                        &relation_type_snake,
                        &field_pascal
                    );

                    model_data_struct.add_field(
                        quote! {
                            #[serde(
                                rename = #field_string,
                                skip_serializing_if = "Result::is_err",
                                default = "prisma_client_rust::serde::default_field_not_fetched",
                                with = "prisma_client_rust::serde::required_relation"
                            )]
                            pub #field_snake: RelationResult<Vec<super::#relation_type_snake::Data>>
                        }
                    );
                } else {
                    field_query_module.add_method(quote! {
                        pub struct Fetch {
                            args: #relation_type_snake::UniqueArgs
                        }
                        
                        impl Fetch {
                            #with_fn
                        }
                        
                        impl From<Fetch> for WithParam {
                            fn from(fetch: Fetch) -> Self {
                                WithParam::#field_pascal(fetch.args)
                            }
                        }
                        
                        pub fn fetch() -> Fetch {
                            Fetch {
                                args: #relation_type_snake::UniqueArgs::new()
                            }
                        }

                        pub fn link<T: From<Link>>(value: #relation_type_snake::UniqueWhereParam) -> T {
                            Link(value).into()
                        }
                    });

                    field_query_module.add_struct(quote! {
                        pub struct Link(#relation_type_snake::UniqueWhereParam);

                        impl From<Link> for SetParam {
                            fn from(value: Link) -> Self {
                                Self::#link_variant(value.0)
                            }
                        }
                    });

                    model_set_params.add_variant(
                        quote!(#link_variant(super::#relation_type_snake::UniqueWhereParam)),
                        quote! {
                            SetParam::#link_variant(where_param) => (
                                #field_string.to_string(),
                                PrismaValue::Object(
                                    vec![(
                                        "connect".to_string(),
                                        PrismaValue::Object(
                                            transform_equals(
                                                vec![Into::<super::#relation_type_snake::WhereParam>::into(where_param)]
                                            )
                                        )
                                    )]
                                )
                            )
                        }
                    );

                    if !field.is_required {
                        field_query_module.add_method(quote! {
                            pub fn unlink() -> SetParam {
                                SetParam::#unlink_variant
                            }
                        });

                        model_set_params.add_variant(
                            quote!(#unlink_variant),
                            quote! {
                                SetParam::#unlink_variant => (
                                    #field_string.to_string(),
                                    PrismaValue::Object(
                                        vec![(
                                            "disconnect".to_string(),
                                            PrismaValue::Boolean(true)
                                        )]
                                    )
                                )
                            },
                        );
                    }

                    model_with_params.add_single_variant(
                        field_string,
                        &relation_type_snake,
                        &field_pascal
                    );
                    
                    let struct_field_type = if field.is_required {
                        quote!(Box<super::#relation_type_snake::Data>)
                    } else {
                        quote!(Option<Box<super::#relation_type_snake::Data>>)
                    };
                    
                    let serde_attr = if field.is_required {
                        quote! {
                            #[serde(
                                rename = #field_string,
                                skip_serializing_if = "Result::is_err",
                                default = "prisma_client_rust::serde::default_field_not_fetched",
                                with = "prisma_client_rust::serde::required_relation"
                            )]
                        }
                    } else {
                        quote! {
                           #[serde(
                                rename = #field_string,
                                skip_serializing_if = "Result::is_err", 
                                default = "prisma_client_rust::serde::default_field_not_fetched",
                                with = "prisma_client_rust::serde::optional_single_relation"
                            )]
                        }
                    };
                    
                    model_data_struct.add_field(
                        quote! {
                            #serde_attr
                            pub #field_snake: RelationResult<#struct_field_type>
                        }
                    );
                };

                if field.required_on_create() {
                    model_actions.push_required_arg(
                        &field_snake,
                        format_ident!("Link")
                   );
                }
            }
            // Scalar actions
            else {
                let field_set_variant = SetParams::field_set_variant(&field.name);

                if !field.prisma {
                    let converter = field.field_type.to_query_value(&format_ident!("value"), field.is_list);
                    let (field_set_variant_type, field_content) = if field.is_list {
                        (
                            quote!(Vec<#field_type>),
                            converter,
                        )
                    } else {
                        if field.is_required {
                            (quote!(#field_type), converter)
                        } else {
                            (quote!(Option<#field_type>), quote!(value.map(|value| #converter).unwrap_or(PrismaValue::Null)))
                        }
                    };

                    field_query_module.add_method(quote! {
                        pub fn set<T: From<Set>>(value: #field_set_variant_type) -> T {
                            Set(value).into()
                        }
                    });

                    field_query_module.add_struct(quote! {
                        pub struct Set(#field_set_variant_type);
                        impl From<Set> for SetParam {
                            fn from(value: Set) -> Self {
                                Self::#field_set_variant(value.0)
                            }
                        }
                    });
                    
                    
                    model_set_params.add_variant(
                        quote!(#field_set_variant(#field_set_variant_type)),
                        quote! {
                            SetParam::#field_set_variant(value) => (
                                #field_string.to_string(),
                                #field_content
                            )
                        },
                    );

                    let equals_variant_name = format_ident!("{}Equals", &field_pascal);
                    let equals_variant = quote!(#equals_variant_name(#field_set_variant_type));
                    let type_as_query_value = field.field_type.to_query_value(&format_ident!("value"), field.is_list);
                    
                    let type_as_query_value = if field.is_required {
                        type_as_query_value
                    } else {
                        quote!(value.map(|value| #type_as_query_value).unwrap_or(PrismaValue::Null))
                    };

                    let match_arm = quote! {
                        Self::#equals_variant_name(value) => (
                            #field_string.to_string(),
                            SerializedWhereValue::Object(vec![("equals".to_string(), #type_as_query_value)])
                        )
                    };

                    match (field.is_id, field.is_unique, field.is_required)  {
                        (true, _, _) | (_, true, true) => {
                            model_where_params.add_unique_variant(
                                equals_variant.clone(),
                                match_arm,
                                quote! {
                                    UniqueWhereParam::#equals_variant_name(value) => Self::#equals_variant_name(value)
                                },
                                equals_variant
                            );
                            field_query_module.add_method(quote! {
                                pub fn equals<T: From<UniqueWhereParam>>(value: #field_set_variant_type) -> T {
                                    UniqueWhereParam::#equals_variant_name(value).into()
                                }
                            });
                        }
                        (_, true, false) => {
                            model_where_params.add_optional_unique_variant(
                                equals_variant,
                                match_arm,
                                quote! {
                                    UniqueWhereParam::#equals_variant_name(value) => Self::#equals_variant_name(Some(value))
                                },
                                quote!(#equals_variant_name(#field_type)),
                                &field_type,
                                &equals_variant_name,
                                quote!(#field_snake::Set)
                            );
                            
                            field_query_module.add_method(quote! {
                                pub fn equals<A, T: prisma_client_rust::traits::FromOptionalUniqueArg<Set, Arg = A>>(value: A) -> T {
                                    T::from_arg(value)
                                }
                            });
                        },
                        (_, _, _) => {
                            model_where_params.add_variant(equals_variant, match_arm);
                            field_query_module.add_method(quote! {
                                pub fn equals(value: #field_set_variant_type) -> WhereParam {
                                    WhereParam::#equals_variant_name(value).into()
                                }
                            });
                        }
                    };

                    // Pagination
                    field_query_module.add_method(quote! {
                        pub fn order(direction: Direction) -> OrderByParam {
                            OrderByParam::#field_pascal(direction)
                        }
                    });
                    
                    if field.is_id || field.is_unique {
                        field_query_module.add_method(quote! {
                            pub fn cursor(cursor: #field_type) -> Cursor {
                                Cursor::#field_pascal(cursor)
                            }
                        });
                    }

                    model_data_struct.add_field(match (field.is_list, field.is_required) {
                        (true, _) => quote! {
                            #[serde(rename = #field_string)]
                            pub #field_snake: Vec<#field_type>
                        },
                        (_, true) => quote! {
                            #[serde(rename = #field_string)]
                            pub #field_snake: #field_type
                        },
                        (_, false) => quote! {
                            #[serde(rename = #field_string)]
                            pub #field_snake: Option<#field_type>
                        },
                    });
                }
                
                let write_type = root
                    .ast
                    .as_ref()
                    .unwrap()
                    .write_filter(field.field_type.string(), field.is_list);

                if let Some(write_type) = write_type {
                    for method in &write_type.methods {
                        let typ = match method.typ.string() {
                            "" => field.field_type.tokens(),
                            _ => method.typ.tokens(),
                        };

                        let method_name_snake = format_ident!("{}", method.name.to_case(Case::Snake));

                        let typ = if method.is_list {
                            quote!(Vec<#typ>)
                        } else { typ };
                        
                        let query_value_converter = method.typ.to_query_value(&format_ident!("value"), method.is_list);

                        let variant_name = format_ident!("{}{}", method.name.to_case(Case::Pascal), field_pascal);

                        field_query_module.add_method(quote! {
                            pub fn #method_name_snake(value: #typ) -> SetParam {
                                SetParam::#variant_name(value)
                            }
                        });
                        
                        let method_action = &method.action;
                        model_set_params.add_variant(
                            quote!(#variant_name(#typ)),
                            quote! {
                                SetParam::#variant_name(value) => (
                                    #field_string.to_string(),
                                    PrismaValue::Object(
                                        vec![(
                                            #method_action.to_string(),
                                            #query_value_converter
                                        )]
                                    )
                                )
                            }
                        );
                    }
                }

                model_order_by_params.add_variant(field_string, &field_pascal);

                model_pagination_params.add_variant(
                    field_string,
                    &field_pascal,
                    &field.field_type
                );

                if field.required_on_create() {
                    model_actions.push_required_arg(
                        &field_snake,
                        format_ident!("Set")
                    );
                }
            }

            if let Some(read_type) = root
                .ast
                .as_ref()
                .unwrap()
                .read_filter(field.field_type.string(), field.is_list)
            {
                for method in &read_type.methods {
                    let typ = match method.typ.string() {
                        "" => field.field_type.tokens(),
                        _ => method.typ.tokens(),
                    };

                    let method_name = format_ident!("{}", method.name.to_case(Case::Snake));
                    let variant_name =
                        format_ident!("{}{}", &field_pascal, method.name.to_case(Case::Pascal));
                    let method_action_string = &method.action;

                    let field_name = if field.prisma {
                        format!("_{}", &field.name)
                    } else {
                        field.name.to_string()
                    };

                    let (typ, value_as_query_value) = if method.is_list {
                        let prisma_value_converter = method.typ.to_prisma_value(&format_ident!("v"));
                        
                        (
                            quote!(Vec<#typ>),
                            quote! {
                                PrismaValue::List(
                                    value
                                        .into_iter()
                                        .map(|v| #prisma_value_converter)
                                        .collect()
                                )
                            },
                        )
                    } else {
                        let as_prisma_value = method.typ.to_prisma_value(&format_ident!("value"));
                        (
                            typ,
                            quote!(#as_prisma_value.into()),
                        )
                    };

                    model_where_params.add_variant(
                        quote!(#variant_name(#typ)),
                        quote! {
                            Self::#variant_name(value) => (
                                #field_name.to_string(),
                                SerializedWhereValue::Object(vec![(#method_action_string.to_string(), #value_as_query_value)])
                            )
                        },
                    );

                    field_query_module.add_method(quote! {
                        pub fn #method_name(value: #typ) -> WhereParam {
                            WhereParam::#variant_name(value)
                        }
                    });
                }
            }

            model_query_module.add_field_module(field_query_module);
        }

        let Actions {
            create_args,
            create_args_tuple_types,
            create_args_destructured,
            create_args_params_pushes
        } = &model_actions;
        
        let data_struct = model_data_struct.quote();
        let with_params = model_with_params.quote();
        let set_params = model_set_params.quote();
        let order_by_params = model_order_by_params.quote();
        let pagination_params = model_pagination_params.quote();
        let outputs_fn = model_outputs.quote();
        let query_modules = model_query_module.quote();
        let where_params = model_where_params.quote();

        quote! {
            pub mod #model_name_snake {
                use super::*;
                
                #query_modules
                
                #outputs_fn

                #data_struct

                #with_params

                #set_params

                #order_by_params

                #pagination_params

                #where_params

                pub type UniqueArgs = prisma_client_rust::UniqueArgs<WithParam>;
                pub type ManyArgs = prisma_client_rust::ManyArgs<WhereParam, WithParam, OrderByParam, Cursor>;
                
                pub type Create<'a> = prisma_client_rust::Create<'a, SetParam, WithParam, Data>;
                pub type FindUnique<'a> = prisma_client_rust::FindUnique<'a, WhereParam, WithParam, SetParam, Data>;
                pub type FindMany<'a> = prisma_client_rust::FindMany<'a, WhereParam, WithParam, OrderByParam, Cursor, SetParam, Data>;
                pub type FindFirst<'a> = prisma_client_rust::FindFirst<'a, WhereParam, WithParam, OrderByParam, Cursor, Data>;
                pub type Update<'a> = prisma_client_rust::Update<'a, WhereParam, SetParam, WithParam, Data>;
                pub type UpdateMany<'a> = prisma_client_rust::UpdateMany<'a, WhereParam, SetParam>;
                pub type Upsert<'a> = prisma_client_rust::Upsert<'a, WhereParam, SetParam, WithParam, Data>;
                pub type Delete<'a> = prisma_client_rust::Delete<'a, WhereParam, WithParam, Data>;
                pub type DeleteMany<'a> = prisma_client_rust::DeleteMany<'a, WhereParam>;
              
                pub struct Actions<'a> {
                    pub client: &'a PrismaClient,
                }

                impl<'a> Actions<'a> {
                    pub fn create(&self, #(#create_args)* mut _params: Vec<SetParam>) -> Create {
                        #(#create_args_params_pushes)*

                        Create::new(
                            QueryContext::new(&self.client.executor, self.client.query_schema.clone()),
                            QueryInfo::new(#model_name_string, _outputs()),
                            _params
                        )
                    }

                    pub fn find_unique(&self, param: UniqueWhereParam) -> FindUnique {
                        FindUnique::new(
                            QueryContext::new(&self.client.executor, self.client.query_schema.clone()),
                            QueryInfo::new(#model_name_string, _outputs()),
                            param.into()
                        )
                    }

                    pub fn find_first(&self, params: Vec<WhereParam>) -> FindFirst {
                        FindFirst::new(
                            QueryContext::new(&self.client.executor, self.client.query_schema.clone()),
                            QueryInfo::new(#model_name_string, _outputs()),
                            params
                        )
                    }

                    pub fn find_many(&self, params: Vec<WhereParam>) -> FindMany {
                        FindMany::new(
                            QueryContext::new(&self.client.executor, self.client.query_schema.clone()),
                            QueryInfo::new(#model_name_string, _outputs()),
                            params
                        )
                    }

                    pub fn upsert(&self, _where: UniqueWhereParam, _create: (#(#create_args_tuple_types)* Vec<SetParam>), _update: Vec<SetParam>) -> Upsert {
                        let (
                            #(#create_args_destructured)*
                            mut _params
                        ) = _create;
                        
                        #(#create_args_params_pushes)*
                        
                        Upsert::new( 
                            QueryContext::new(&self.client.executor, self.client.query_schema.clone()),
                            QueryInfo::new(#model_name_string, _outputs()),
                            _where.into(),
                            _params,
                            _update
                        )
                    }
                }
            }
        }
    }).collect()
}
