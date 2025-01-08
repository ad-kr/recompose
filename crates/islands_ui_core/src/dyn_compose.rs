use crate::{AnyCompose, Compose, Scope};
use std::{any::Any, any::TypeId, sync::Arc};

#[derive(Clone)]
pub struct DynCompose {
    type_id: TypeId,
    compose: Arc<dyn AnyCompose>,
}

impl DynCompose {
    pub fn new(compose: impl Compose + 'static) -> Self {
        Self {
            type_id: compose.type_id(),
            compose: Arc::new(compose),
        }
    }
}

impl Compose for DynCompose {
    fn compose<'a>(&self, cx: &mut Scope) -> impl Compose + 'a {
        let type_id = cx.use_state(self.type_id);

        let create_new_scope = |cx: &mut Scope| {
            let mut scope = Scope::new(self.compose.clone(), cx.id);
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
}
