use std::{
    collections::LinkedList,
    ops::{Deref, DerefMut},
};

pub struct ExtendedLinkedList<T>(LinkedList<T>);

impl<T> Deref for ExtendedLinkedList<T> {
    type Target = LinkedList<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for ExtendedLinkedList<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> ExtendedLinkedList<T> {
    pub fn from<const N: usize>(list: [T; N]) -> Self {
        Self(LinkedList::from(list))
    }

    pub fn from_vec(list: Vec<T>) -> Self {
        Self(list.into_iter().collect())
    }

    pub fn push_at(&mut self, index: usize, element: T) {
        let mut split = self.split_off(index);
        self.push_back(element);
        self.append(&mut split);
    }

    pub fn remove_at(&mut self, index: usize) -> Option<T> {
        let mut split_list = self.split_off(index);
        let result = split_list.pop_front();
        self.append(&mut split_list);

        result
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.0.iter().nth(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.0.iter_mut().nth(index)
    }
}
