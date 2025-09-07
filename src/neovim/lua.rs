#[cfg(not(test))]
macro_rules! include_code {
    ($path:literal) => {
        include_str!($path)
    };
}
#[cfg(test)]
macro_rules! include_code {
    ($path:literal) => {
        concat!("return loadfile(\"src/neovim/", $path, "\")(...)")
    };
}

pub(crate) const SCRIPT_LSP_MAKE_TEXT_DOCUMENT_PARAMS: &str =
    include_code!("lua/lsp_make_text_document_params.lua");
