use std::{
    alloc::{alloc, Layout},
    ops::{Deref, DerefMut},
};

use crate::Instance;

#[derive(Clone, Debug)]
pub struct InstanceEnv {
    ptr: *mut Instance,
}

unsafe impl Sync for InstanceEnv {}

unsafe impl Send for InstanceEnv {}

impl InstanceEnv {
    pub(crate) fn set(&mut self, ins: *mut Instance) {
        self.ptr = ins
    }
}

impl Default for InstanceEnv {
    fn default() -> Self {
        unsafe { InstanceEnv { ptr: alloc(Layout::new::<Instance>()) as *mut Instance } }
    }
}

impl From<&mut Instance> for InstanceEnv {
    fn from(ins: &mut Instance) -> Self {
        InstanceEnv { ptr: ins as *mut Instance }
    }
}

impl Deref for InstanceEnv {
    type Target = Instance;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl DerefMut for InstanceEnv {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr }
    }
}
