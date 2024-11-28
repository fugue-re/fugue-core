use std::path::PathBuf;

use fugue_ir::LanguageDB;

use proc_macro::TokenStream;
use proc_macro_error::{abort_call_site, proc_macro_error};

use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, LitStr, Token};

struct CodegenConfig {
    root: PathBuf,
    lang: String,
}

impl Parse for CodegenConfig {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let root_tok = input.parse::<LitStr>()?;
        let _ = input.parse::<Token![,]>()?;
        let lang_tok = input.parse::<LitStr>()?;

        Ok(Self {
            root: PathBuf::from(root_tok.value()),
            lang: lang_tok.value(),
        })
    }
}

#[proc_macro]
#[proc_macro_error]
pub fn generate(input: TokenStream) -> TokenStream {
    let config = parse_macro_input!(input as CodegenConfig);
    let Ok(languagedb) = LanguageDB::from_directory_with(&config.root, true) else {
        abort_call_site!(
            "cannot parse language database at `{}`",
            config.root.display()
        );
    };

    let Ok(Some(tbuilder)) = languagedb.lookup_str(&config.lang) else {
        abort_call_site!(
            "cannot find language definition matching specification `{}`",
            config.lang
        );
    };

    let translator = match tbuilder.build_with(true) {
        Ok(translator) => translator,
        Err(e) => abort_call_site!("cannot build translator for `{}`: {}", config.lang, e),
    };

    match fugue_lifter::from_translator(&translator) {
        Ok(tokens) => tokens.into(),
        Err(e) => abort_call_site!("cannot generate code for `{}`: {}", config.lang, e),
    }
}
