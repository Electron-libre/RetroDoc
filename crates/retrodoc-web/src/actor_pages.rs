//! The handlers of the actors pages (`specs/web-actors.md`): the index,
//! `/actors`, and the page of an actor, `/actors/{id}`.

use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;

use crate::actors::{ActorSection, ActorsPage};
use crate::pages::{no_run_page, not_found, render};
use crate::texts::WebTexts;
use crate::view::{Shell, Site};

#[derive(Template)]
#[template(path = "actors.html")]
struct ActorsTemplate<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
    page: &'a ActorsPage,
}

#[derive(Template)]
#[template(path = "actor.html")]
struct ActorTemplate<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
    kind: &'static str,
    actor: &'a ActorSection,
}

pub async fn actors(State(site): State<Arc<Site>>) -> Response {
    let Some(page) = site.actors() else {
        return no_run_page(&site);
    };
    render(
        StatusCode::OK,
        &ActorsTemplate {
            t: site.texts(),
            shell: site.shell(),
            page,
        },
    )
}

pub async fn actor(State(site): State<Arc<Site>>, Path(id): Path<String>) -> Response {
    let Some(page) = site.actors() else {
        return no_run_page(&site);
    };
    let Some((kind, actor)) = page.actor(&id) else {
        return not_found(State(site)).await;
    };
    render(
        StatusCode::OK,
        &ActorTemplate {
            t: site.texts(),
            shell: site.shell(),
            kind,
            actor,
        },
    )
}
