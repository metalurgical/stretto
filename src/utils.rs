/*
 * Copyright 2021 Al Liu (https://github.com/al8n/stretto). Licensed under Apache-2.0.
 *
 * Copy some code from Dashmap(https://github.com/xacrimon/dashmap). Licensed under MIT.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
use crate::store::StoreItem;
use parking_lot::{RwLockReadGuard, RwLockWriteGuard};
use std::cell::UnsafeCell;
use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::convert::TryInto;
use std::hash::BuildHasher;
use std::ptr::NonNull;
use std::fmt::{Display, Formatter, Debug};

#[allow(dead_code)]
pub struct ValueRef<'a, V, S = RandomState> {
    guard: RwLockReadGuard<'a, HashMap<u64, StoreItem<V>, S>>,
    val: &'a V,
}

unsafe impl<'a, V: Send, S: BuildHasher> Send for ValueRef<'a, V, S> {}

unsafe impl<'a, V: Send + Sync, S: BuildHasher> Sync for ValueRef<'a, V, S> {}

impl<'a, V, S: BuildHasher> ValueRef<'a, V, S> {
    pub(crate) fn new(
        guard: RwLockReadGuard<'a, HashMap<u64, StoreItem<V>, S>>,
        val: &'a V,
    ) -> Self {
        Self { guard, val }
    }

    pub fn value(&self) -> &V {
        self.val
    }

    pub fn release(self) {
        drop(self)
    }
}

impl<'a, V: Copy, S: BuildHasher> ValueRef<'a, V, S> {
    /// Get the value and drop the inner RwLockReadGuard.
    pub fn read(self) -> V {
        let v = *self.val;
        drop(self);
        v
    }
}

impl<'a, V, S: BuildHasher> AsRef<V> for ValueRef<'a, V, S> {
    fn as_ref(&self) -> &V {
        self.value()
    }
}

impl<'a, V: Debug, S: BuildHasher> Debug for ValueRef<'a, V, S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.val)
    }
}

impl<'a, V: Display, S: BuildHasher> Display for ValueRef<'a, V, S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.val)
    }
}

#[allow(dead_code)]
pub struct ValueRefMut<'a, V, S = RandomState> {
    guard: RwLockWriteGuard<'a, HashMap<u64, StoreItem<V>, S>>,
    val: &'a mut V,
}

unsafe impl<'a, V: Send, S: BuildHasher> Send for ValueRefMut<'a, V, S> {}

unsafe impl<'a, V: Send + Sync, S: BuildHasher> Sync for ValueRefMut<'a, V, S> {}

impl<'a, V, S: BuildHasher> ValueRefMut<'a, V, S> {
    pub(crate) fn new(
        guard: RwLockWriteGuard<'a, HashMap<u64, StoreItem<V>, S>>,
        val: &'a mut V,
    ) -> Self {
        Self { guard, val }
    }

    pub fn value(&self) -> &V {
        self.val
    }

    pub fn value_mut(&mut self) -> &mut V {
        self.val
    }

    /// Set the value
    pub fn write(&mut self, val: V) {
        *self.val = val
    }

    /// Set the value, and release the inner `RwLockWriteGuard` automatically
    pub fn write_once(mut self, val: V) {
        *self.val = val;
        self.release();
    }

    pub fn release(self) {
        drop(self)
    }
}

impl<'a, V: Clone, S: BuildHasher> ValueRefMut<'a, V, S> {
    pub fn clone_inner(&self) -> V {
        self.val.clone()
    }
}

impl<'a, V: Copy, S: BuildHasher> ValueRefMut<'a, V, S> {
    pub fn read(self) -> V {
        let v = *self.val;
        drop(self);
        v
    }
}

impl<'a, V: Debug, S: BuildHasher> Debug for ValueRefMut<'a, V, S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.val)
    }
}

impl<'a, V: Display, S: BuildHasher> Display for ValueRefMut<'a, V, S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.val)
    }
}

impl<'a, V, S: BuildHasher> AsRef<V> for ValueRefMut<'a, V, S> {
    fn as_ref(&self) -> &V {
        self.value()
    }
}

impl<'a, V, S: BuildHasher> AsMut<V> for ValueRefMut<'a, V, S> {
    fn as_mut(&mut self) -> &mut V {
        self.value_mut()
    }
}

#[repr(transparent)]
pub struct SharedValue<T> {
    value: UnsafeCell<T>,
}

impl<T: Clone> Clone for SharedValue<T> {
    fn clone(&self) -> Self {
        let inner = self.get().clone();

        Self {
            value: UnsafeCell::new(inner),
        }
    }
}

unsafe impl<T: Send> Send for SharedValue<T> {}

unsafe impl<T: Sync> Sync for SharedValue<T> {}

impl<T> SharedValue<T> {
    /// Create a new `SharedValue<T>`
    pub const fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(value),
        }
    }

    /// Get a shared reference to `T`
    pub fn get(&self) -> &T {
        unsafe { &*self.value.get() }
    }

    /// Get an unique reference to `T`
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.value.get() }
    }

    /// Unwraps the value
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }

    /// Get a mutable raw pointer to the underlying value
    pub(crate) fn as_ptr(&self) -> *mut T {
        self.value.get()
    }
}

pub(crate) fn vec_to_array<T, const N: usize>(v: Vec<T>) -> [T; N] {
    v.try_into()
        .unwrap_or_else(|v: Vec<T>| panic!("Expected a Vec of length {} but it was {}", N, v.len()))
}

/// # Safety
///
/// Requires that you ensure the reference does not become invalid.
/// The object has to outlive the reference.
pub(crate) unsafe fn change_lifetime_const<'a, 'b, T>(x: &'a T) -> &'b T {
    &*(x as *const T)
}

// TODO: should use SharedNonNull to replace Arc?
#[repr(transparent)]
#[allow(dead_code)]
pub(crate) struct SharedNonNull<T: ?Sized> {
    ptr: NonNull<T>,
}

impl<T> SharedNonNull<T> {
    #[allow(dead_code)]
    pub fn new(ptr: *mut T) -> Self {
        unsafe {
            Self {
                ptr: NonNull::new_unchecked(ptr),
            }
        }
    }

    #[allow(dead_code)]
    pub unsafe fn as_ref(&self) -> &T {
        self.ptr.as_ref()
    }
}

impl<T: ?Sized> Copy for SharedNonNull<T> {}

impl<T: ?Sized> Clone for SharedNonNull<T> {
    fn clone(&self) -> Self {
        *self
    }
}

unsafe impl<T> Send for SharedNonNull<T> {}
unsafe impl<T> Sync for SharedNonNull<T> {}
