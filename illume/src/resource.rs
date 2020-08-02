use std::{any::Any, fmt::Debug, marker::PhantomData, sync::Arc};

pub trait ResourceTrait: Sized {
    /// Resource info.
    type Info: Clone + Debug + Send + Sync + 'static;

    fn from_handle(handle: Handle<Self>) -> Self;

    fn handle(&self) -> &Handle<Self>;
}

pub trait Specific<R>: Debug + Send + Sync + 'static {}

/// Type erased safe-ish resource type.
#[derive(Debug)]
struct ResourceData<R: ResourceTrait, S: ?Sized> {
    marker: PhantomData<R>,
    info: R::Info,
    specific: S,
}

trait AnyResource: Any + Send + Sync {}

impl dyn AnyResource + 'static {
    pub fn is<T>(&self) -> bool
    where
        T: 'static,
    {
        self.type_id() == std::any::TypeId::of::<T>()
    }

    fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: 'static,
    {
        if self.is::<T>() {
            Some(unsafe { self.downcast_ref_unchecked() })
        } else {
            None
        }
    }

    unsafe fn downcast_ref_unchecked<T>(&self) -> &T
    where
        T: 'static,
    {
        &*(self as *const Self as *const T)
    }
}

impl<T> AnyResource for T where T: 'static + Send + Sync {}

#[repr(transparent)]
pub struct Handle<R: ResourceTrait>(Arc<ResourceData<R, dyn AnyResource>>);

impl<R> Handle<R>
where
    R: ResourceTrait,
{
    /// Creates safe-ish handle out of implementation specific value and
    /// implementation-agnostic info.
    pub fn new<S: Specific<R>>(specific: S, info: R::Info) -> Self {
        Handle(Arc::new(ResourceData {
            specific,
            info,
            marker: PhantomData,
        }))
    }

    /// Returns raw handle value without checking type and ownership.
    pub fn specific_ref<S: Specific<R>>(&self) -> Option<&S> {
        self.0.specific.downcast_ref()
    }

    /// Returns payload that was supplied to `Handle::new`.
    /// Does not check type of the payload.
    ///
    /// # Safety
    ///
    /// Caller is responsible to ensure that specific ref is of required type.
    /// i.e. `specific_ref` with same type parameter must not fail.
    pub unsafe fn specific_ref_unchecked<S: Specific<R>>(&self) -> &S {
        self.0.specific.downcast_ref_unchecked()
    }

    /// Returns an information that was supplied to `Handle::new`.
    pub fn info(&self) -> &R::Info {
        &self.0.info
    }
}

impl<R: ResourceTrait> std::cmp::PartialEq for Handle<R> {
    fn eq(&self, rhs: &Self) -> bool {
        std::ptr::eq(&*self.0, &*rhs.0)
    }
}

impl<R: ResourceTrait> std::cmp::Eq for Handle<R> {}

impl<R: ResourceTrait> std::hash::Hash for Handle<R> {
    fn hash<H>(&self, state: &mut H)
    where
        H: std::hash::Hasher,
    {
        std::hash::Hash::hash(&(&*self.0 as *const _), state);
    }
}

impl<R: ResourceTrait> std::fmt::Debug for Handle<R> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if fmt.alternate() {
            fmt.debug_struct("Handle")
                .field("ptr", &(&*self.0 as *const _))
                .field("info", self.info())
                .finish()
        } else {
            std::fmt::Debug::fmt(&(&*self.0 as *const _), fmt)
        }
    }
}

impl<R: ResourceTrait> Clone for Handle<R> {
    fn clone(&self) -> Self {
        Handle(self.0.clone())
    }
}
