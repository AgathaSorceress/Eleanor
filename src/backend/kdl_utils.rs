//! Helper functions for interacting with KDL documents.

use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};

pub(crate) trait KdlNodeExt {
    fn first_arg(&self) -> Option<KdlValue>;

    fn add_arg<T: Into<KdlValue>>(&mut self, value: T, ty: Option<&str>) -> &mut Self;

    fn add_child(&mut self, node: KdlNode) -> &mut Self;

    fn set_param<T: Into<KdlValue>>(&mut self, key: &str, value: T) -> &mut Self;

    fn with_arg<T: Into<KdlValue>>(name: &str, value: T) -> Self;
}

impl KdlNodeExt for KdlNode {
    fn first_arg(&self) -> Option<KdlValue> {
        self.entries()
            .iter()
            .find(|node| node.name().is_none())
            .map(KdlEntry::value)
            .cloned()
    }

    fn add_arg<T: Into<KdlValue>>(&mut self, value: T, ty: Option<&str>) -> &mut Self {
        let mut arg = KdlEntry::new(value);
        if let Some(ty) = ty {
            arg.set_ty(ty);
        }
        self.push(arg);

        self
    }

    fn add_child(&mut self, node: KdlNode) -> &mut Self {
        if let Some(children) = self.children_mut() {
            children.add_child(node);
        } else {
            self.set_children(KdlDocument::new().add_child(node).clone());
        }

        self
    }

    fn set_param<T: Into<KdlValue>>(&mut self, key: &str, value: T) -> &mut Self {
        self.insert(key, value);

        self
    }

    fn with_arg<T: Into<KdlValue>>(name: &str, value: T) -> Self {
        KdlNode::new(name).add_arg(value, None).clone()
    }
}

pub(crate) trait KdlDocumentExt {
    fn get_bool_or(&self, name: &str, default: bool) -> bool;

    fn get_u32_or(&self, name: &str, default: u32) -> u32;

    fn get_f64_or(&self, name: &str, default: f64) -> f64;

    fn get_children_or(&self, name: &str, default: KdlDocument) -> KdlDocument;

    fn add_child(&mut self, node: KdlNode) -> &mut Self;
}

impl KdlDocumentExt for KdlDocument {
    fn get_bool_or(&self, name: &str, default: bool) -> bool {
        self.get(name)
            .and_then(KdlNode::first_arg)
            .and_then(|v| v.as_bool())
            .unwrap_or(default)
    }

    fn get_u32_or(&self, name: &str, default: u32) -> u32 {
        self.get(name)
            .and_then(KdlNode::first_arg)
            .and_then(|v| v.as_i64())
            .and_then(|v| v.try_into().ok())
            .unwrap_or(default)
    }

    fn get_f64_or(&self, name: &str, default: f64) -> f64 {
        self.get(name)
            .and_then(KdlNode::first_arg)
            .and_then(|v| v.as_f64())
            .unwrap_or(default)
    }

    fn get_children_or(&self, name: &str, default: KdlDocument) -> KdlDocument {
        self.get(name)
            .and_then(|v| v.children().cloned())
            .unwrap_or(default)
    }

    fn add_child(&mut self, node: KdlNode) -> &mut Self {
        self.nodes_mut().push(node);

        self
    }
}
