use crate::{
    AppState, authenticated_user,
    pages::{self, PageMutationError},
};
use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header, request::Parts, uri::Authority},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig},
    service::RequestContext,
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde_json::json;

#[derive(serde::Deserialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
struct PublishPage {
    markdown: String,
    slug: Option<String>,
}

#[derive(Clone)]
struct PageletMcp {
    state: AppState,
    tool_router: ToolRouter<Self>,
}

#[derive(serde::Deserialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
struct ReadPage {
    /// The immutable Page ID returned by list_pages.
    id: i64,
}

#[derive(serde::Deserialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
struct EditPage {
    /// The immutable Page ID returned by list_pages.
    id: i64,
    /// The last-read Page version returned by read_page.
    version: i64,
    markdown: Option<String>,
    slug: Option<String>,
}

#[tool_router]
impl PageletMcp {
    #[tool(
        description = "Read the Markdown and current version of a manageable Pagelet page by its immutable Page ID.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn read_page(
        &self,
        Parameters(ReadPage { id }): Parameters<ReadPage>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let headers = request_headers(&context)?;
        let result = pages::read_management_page(&self.state, id, headers);
        Ok(match result {
            Ok(page) => CallToolResult::structured(pages::read_page_content(page)),
            Err(status) => tool_error(status),
        })
    }

    #[tool(
        description = "List manageable Pagelet page metadata by slug search and page number.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_pages(
        &self,
        Parameters(query): Parameters<pages::PageQuery>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let headers = request_headers(&context)?;
        let result = authenticated_user(&self.state, headers)
            .and_then(|account| pages::list_pages(&self.state, &account, query));
        Ok(match result {
            Ok(result) => CallToolResult::structured(result),
            Err(status) => tool_error(status),
        })
    }

    #[tool(
        description = "Immediately publish a public Markdown page. Omit the slug to generate one.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn publish_page(
        &self,
        Parameters(input): Parameters<PublishPage>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let headers = request_headers(&context)?;
        Ok(page_mutation_result(
            &self.state,
            pages::create_page(
                &self.state,
                headers,
                pages::CreatePage {
                    markdown: input.markdown,
                    slug: input.slug,
                    access_password: None,
                },
            ),
        ))
    }

    #[tool(
        description = "Immediately edit a manageable Pagelet page. Provide the last-read version and at least one of markdown or slug.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn edit_page(
        &self,
        Parameters(input): Parameters<EditPage>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let headers = request_headers(&context)?;
        Ok(page_mutation_result(
            &self.state,
            pages::edit_page(
                &self.state,
                input.id,
                headers,
                pages::EditPage {
                    version: input.version,
                    markdown: input.markdown,
                    slug: input.slug,
                    access_password: None,
                },
            ),
        ))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for PageletMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("pagelet", env!("CARGO_PKG_VERSION")))
    }
}

fn request_headers(context: &RequestContext<RoleServer>) -> Result<&HeaderMap, ErrorData> {
    context
        .extensions
        .get::<Parts>()
        .map(|parts| &parts.headers)
        .ok_or_else(|| ErrorData::internal_error("Missing HTTP request context", None))
}

fn tool_error(status: StatusCode) -> CallToolResult {
    let code = match status {
        StatusCode::BAD_REQUEST => "invalid_arguments",
        StatusCode::UNAUTHORIZED => "unauthorized",
        StatusCode::FORBIDDEN => "forbidden",
        StatusCode::NOT_FOUND => "not_found",
        StatusCode::PAYLOAD_TOO_LARGE => "payload_too_large",
        _ => "internal_error",
    };
    CallToolResult::structured_error(json!({"code": code}))
}

fn page_mutation_result(
    state: &AppState,
    result: Result<pages::Page, PageMutationError>,
) -> CallToolResult {
    match result {
        Ok(page) => {
            let url = match state.deployment.external_url() {
                Some(base) => format!(
                    "{}://{}/{}",
                    base.scheme_str().expect("External URL has a scheme"),
                    base.authority().expect("External URL has an authority"),
                    page.slug,
                ),
                None => format!("/{}", page.slug),
            };
            CallToolResult::structured(
                json!({"id": page.id, "slug": page.slug, "version": page.version, "url": url}),
            )
        }
        Err(PageMutationError::Conflict("stale_version")) => CallToolResult::structured_error(
            json!({"code": "stale_version", "message": "Reread the page with read_page and reconcile your changes before submitting another edit."}),
        ),
        Err(PageMutationError::Conflict(code)) => {
            CallToolResult::structured_error(json!({"code": code}))
        }
        Err(PageMutationError::Status(status)) => tool_error(status),
    }
}

async fn authorize(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let headers = request.headers();
    let authorization = if headers.contains_key(header::AUTHORIZATION) {
        authenticated_user(&state, headers).map(|_| ())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    };
    let mut response = match authorization {
        Err(StatusCode::UNAUTHORIZED) => (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
        )
            .into_response(),
        Err(status) => status.into_response(),
        Ok(()) if !valid_origin_header(headers) => StatusCode::FORBIDDEN.into_response(),
        Ok(()) => next.run(request).await,
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
}

fn valid_origin_header(headers: &HeaderMap) -> bool {
    let mut origins = headers.get_all(header::ORIGIN).iter();
    let Some(origin) = origins.next() else {
        return true;
    };
    // The SDK compares origin tuples. Reject malformed or repeated headers before that check.
    origins.next().is_none()
        && origin
            .to_str()
            .ok()
            .and_then(|value| value.split_once("://"))
            .and_then(|(_, authority)| authority.parse::<Authority>().ok())
            .is_some_and(|authority| {
                !authority.as_str().contains('@')
                    && (authority.host() == authority.as_str() || authority.port_u16().is_some())
            })
}

pub(crate) fn routes(state: AppState) -> Router<AppState> {
    let mut config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_max_request_body_bytes(8 * 1024 * 1024)
        .enforce_origin_validation();
    if let Some(url) = state.deployment.external_url() {
        let scheme = url.scheme_str().expect("External URL has a scheme");
        let host = url.host().expect("External URL has a host");
        let port = url
            .port_u16()
            .unwrap_or(if scheme == "https" { 443 } else { 80 });
        config.allowed_hosts.push(host.into());
        config
            .allowed_origins
            .push(format!("{scheme}://{host}:{port}"));
    }
    let server = PageletMcp {
        state: state.clone(),
        tool_router: PageletMcp::tool_router(),
    };
    let service = StreamableHttpService::new(
        move || Ok(server.clone()),
        LocalSessionManager::default().into(),
        config,
    );
    Router::new()
        .route_service("/_/mcp", service)
        .route_layer(middleware::from_fn_with_state(state, authorize))
}
