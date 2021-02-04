use type_map::TypeMap;

pub struct Resources {
    map: TypeMap,
}

impl Resources {
    pub fn new() -> Self {
        Resources {
            map: TypeMap::new(),
        }
    }

    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.map.get()
    }

    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.map.get_mut()
    }

    pub fn get_or_default<T>(&mut self) -> &mut T
    where
        T: Default + 'static,
    {
        self.map.entry::<T>().or_insert_with(T::default)
    }

    pub fn get_or<T>(&mut self, default: T) -> &mut T
    where
        T: 'static,
    {
        self.map.entry::<T>().or_insert(default)
    }

    pub fn get_or_else<T, F>(&mut self, f: F) -> &mut T
    where
        T: 'static,
        F: FnOnce() -> T,
    {
        self.map.entry::<T>().or_insert_with(f)
    }
}
