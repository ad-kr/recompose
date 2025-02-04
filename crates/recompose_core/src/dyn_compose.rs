use crate::{AnyCompose, Compose, Scope};
use std::{any::Any, any::TypeId, sync::Arc};

/// A dynamic composition structure that holds a type-erased composer. This allows for "dynamic dispatch" of the
/// `Compose` trait.
#[derive(Clone)]
pub struct DynCompose {
    /// The `TypeId` is used to determine if the composer has changed type between compositions. If it has, we have to
    /// decompose the previous scope and create a new one.
    type_id: TypeId,
    compose: Arc<dyn AnyCompose>,
}

impl Default for DynCompose {
    fn default() -> Self {
        Self {
            type_id: TypeId::of::<()>(),
            compose: Arc::new(()),
        }
    }
}

impl DynCompose {
    /// Creates a new `DynCompose` instance from a given composer.
    pub fn new(compose: impl Compose + 'static) -> Self {
        Self {
            type_id: compose.type_id(),
            compose: Arc::new(compose),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.type_id == TypeId::of::<()>()
    }
}

impl Compose for DynCompose {
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
        let type_id = cx.use_state(self.type_id);

        let create_new_scope = |cx: &mut Scope| {
            let mut scope = Scope::new(self.compose.clone(), 0);
            self.compose.recompose_scope(&mut scope);
            cx.children.push(scope);
            cx.set_state(&type_id, self.type_id);
        };

        if let Some(ref mut existing_scope) = cx.children.first_mut() {
            if *type_id != self.type_id {
                existing_scope.will_decompose = true;
                create_new_scope(cx);
                return;
            }

            existing_scope.composer = self.compose.clone();
            existing_scope
                .composer
                .clone()
                .recompose_scope(existing_scope);
            return;
        }

        create_new_scope(cx);
    }

    fn ignore_children(&self) -> bool {
        true
    }

    fn name(&self) -> String {
        let inner_name = self.compose.get_name();
        format!("DynCompose({})", inner_name)
    }
}
