use lol_html::html_content::ContentType;
use lol_html::{element, HtmlRewriter, Settings};
use std::collections::HashMap;
use std::fmt::Write;
use std::io;
#[cfg(feature = "axum")]
pub use crate::axum::serve_files_with_script;

#[derive(Debug, Clone)]
/// Map of values that will be provided as environment-variable-like global variables to the frontend.
pub struct FrontedEnvironment(pub HashMap<String, String>);

/// Rewrites HTML to inject a `<script>` tag (which contains global JS variables that act like environment variables)
/// into the `<head>` tag.
pub fn inject_environment_script_tag(
    input: &[u8],
    output: &mut Vec<u8>,
    frontend_env: &FrontedEnvironment,
) -> io::Result<()> {
    let mut script_tag = String::new();
    script_tag.write_str("<script>\n").unwrap();
    // Writes a line with the content `window.KEY = "VALUE";` for every entry
    for (key, value) in &frontend_env.0 {
        script_tag.write_str("window.").unwrap();
        script_tag.write_str(&key).unwrap();
        script_tag.write_str(" = \"").unwrap();
        script_tag.write_str(&value).unwrap();
        script_tag.write_str("\";\n").unwrap();
    }
    script_tag.write_str("</script>").unwrap();

    let mut rewriter = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![element!("head", |el| {
                el.append(&script_tag, ContentType::Html);
                Ok(())
            })],
            ..Settings::default()
        },
        |c: &[u8]| output.extend_from_slice(c),
    );

    rewriter.write(input).unwrap();
    rewriter.end().unwrap();
    Ok(())
}

#[cfg(feature = "axum")]
pub mod axum {
    use ::axum::body::{Body, Bytes, HttpBody};
    use ::axum::headers::HeaderName;
    use ::axum::http::{HeaderValue, Request};
    use ::axum::response::Response;
    use ::axum::{http, BoxError, Extension};
    use http_body::combinators::UnsyncBoxBody;
    use std::convert::Infallible;
    use tower_http::services::{ServeDir, ServeFile};
    use super::*;

    /// Static file handler that injects a script tag with environment variables into HTML files.
    pub async fn serve_files_with_script(
        Extension(frontend_environment): Extension<FrontedEnvironment>,
        req: Request<Body>,
    ) -> Result<Response<UnsyncBoxBody<Bytes, BoxError>>, Infallible> {
        let mut static_files_service =
            ServeDir::new("public").not_found_service(ServeFile::new("public/404.html"));

        let res = static_files_service.try_call(req).await.unwrap();

        let headers = res.headers().clone();
        if headers.get(http::header::CONTENT_TYPE) == Some(&HeaderValue::from_static("text/html")) {
            let mut res = res.map(move |body| {
                let body_bytes = body.map_err(Into::into).boxed_unsync();
                // Inject variables into HTML files
                body_bytes
                    .map_data(move |bytes| {
                        let mut output = Vec::with_capacity(bytes.len() * 2);
                        inject_environment_script_tag(
                            &bytes.as_ref(),
                            &mut output,
                            &frontend_environment,
                        )
                            .unwrap();
                        output.into()
                    })
                    .boxed_unsync()
            });
            res.headers_mut()
                .remove(HeaderName::from_static("content-length"));
            Ok(res)
        } else {
            Ok(res.map(|body| body.map_err(Into::into).boxed_unsync()))
        }
    }
}