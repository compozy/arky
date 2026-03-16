//! Integration coverage for the facade prelude surface.

use std::any::TypeId;

use arky::prelude::*;

fn accepts_provider(_: Option<&dyn Provider>) {}

fn accepts_tool(_: Option<&dyn Tool>) {}

fn accepts_session_store(_: Option<&dyn SessionStore>) {}

#[test]
fn prelude_should_expose_common_types() {
    let _ = TypeId::of::<Agent>();
    let _ = TypeId::of::<AgentBuilder>();
    let _ = TypeId::of::<ToolDescriptor>();
    let _ = TypeId::of::<Message>();
    let _ = TypeId::of::<AgentEvent>();
    let _ = TypeId::of::<ArkyError>();

    accepts_provider(None);
    accepts_tool(None);
    accepts_session_store(None);
}
