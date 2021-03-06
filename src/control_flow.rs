/// Used to tell an operation whether it should exit early or go on as usual.
///
/// This is used when exposing things (like graph traversals or visitors) where
/// you want the user to be able to choose whether to exit early.
/// Having the enum makes it clearer -- no more wondering "wait, what did `false`
/// mean again?" -- and allows including a value.
///
/// # Examples
///
/// Early-exiting from [`Iterator::try_for_each`]:
/// ```
/// #![feature(control_flow_enum)]
/// use std::ops::ControlFlow;
///
/// let r = (2..100).try_for_each(|x| {
///     if 403 % x == 0 {
///         return ControlFlow::Break(x)
///     }
///
///     ControlFlow::Continue(())
/// });
/// assert_eq!(r, ControlFlow::Break(13));
/// ```
///
/// A basic tree traversal:
/// ```no_run
/// #![feature(control_flow_enum)]
/// use std::ops::ControlFlow;
///
/// pub struct TreeNode<T> {
///     value: T,
///     left: Option<Box<TreeNode<T>>>,
///     right: Option<Box<TreeNode<T>>>,
/// }
///
/// impl<T> TreeNode<T> {
///     pub fn traverse_inorder<B>(&self, mut f: impl FnMut(&T) -> ControlFlow<B>) -> ControlFlow<B> {
///         if let Some(left) = &self.left {
///             left.traverse_inorder(&mut f)?;
///         }
///         f(&self.value)?;
///         if let Some(right) = &self.right {
///             right.traverse_inorder(&mut f)?;
///         }
///         ControlFlow::Continue(())
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControlFlow<B = (), C = ()> {
    /// Move on to the next phase of the operation as normal.
    Continue(C),
    /// Exit the operation without running subsequent phases.
    Break(B),
    // Yes, the order of the variants doesn't match the type parameters.
    // They're in this order so that `ControlFlow<A, B>` <-> `Result<B, A>`
    // is a no-op conversion in the `Try` implementation.
}

impl<B, C> ControlFlow<B, C> {
    /// Returns `true` if this is a `Break` variant.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(control_flow_enum)]
    /// use std::ops::ControlFlow;
    ///
    /// assert!(ControlFlow::<i32, String>::Break(3).is_break());
    /// assert!(!ControlFlow::<String, i32>::Continue(3).is_break());
    /// ```
    #[inline]
    pub fn is_break(&self) -> bool {
        matches!(*self, ControlFlow::Break(_))
    }

    /// Returns `true` if this is a `Continue` variant.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(control_flow_enum)]
    /// use std::ops::ControlFlow;
    ///
    /// assert!(!ControlFlow::<i32, String>::Break(3).is_continue());
    /// assert!(ControlFlow::<String, i32>::Continue(3).is_continue());
    /// ```
    #[inline]
    pub fn is_continue(&self) -> bool {
        matches!(*self, ControlFlow::Continue(_))
    }

    /// Converts the `ControlFlow` into an `Option` which is `Some` if the
    /// `ControlFlow` was `Break` and `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(control_flow_enum)]
    /// use std::ops::ControlFlow;
    ///
    /// assert_eq!(ControlFlow::<i32, String>::Break(3).break_value(), Some(3));
    /// assert_eq!(ControlFlow::<String, i32>::Continue(3).break_value(), None);
    /// ```
    #[inline]
    pub fn break_value(self) -> Option<B> {
        match self {
            ControlFlow::Continue(..) => None,
            ControlFlow::Break(x) => Some(x),
        }
    }

    /// Maps `ControlFlow<B, C>` to `ControlFlow<T, C>` by applying a function
    /// to the break value in case it exists.
    #[inline]
    pub fn map_break<T, F>(self, f: F) -> ControlFlow<T, C>
    where
        F: FnOnce(B) -> T,
    {
        match self {
            ControlFlow::Continue(x) => ControlFlow::Continue(x),
            ControlFlow::Break(x) => ControlFlow::Break(f(x)),
        }
    }
}

impl<B> ControlFlow<B, ()> {
    /// It's frequently the case that there's no value needed with `Continue`,
    /// so this provides a way to avoid typing `(())`, if you prefer it.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(control_flow_enum)]
    /// use std::ops::ControlFlow;
    ///
    /// let mut partial_sum = 0;
    /// let last_used = (1..10).chain(20..25).try_for_each(|x| {
    ///     partial_sum += x;
    ///     if partial_sum > 100 { ControlFlow::Break(x) }
    ///     else { ControlFlow::CONTINUE }
    /// });
    /// assert_eq!(last_used.break_value(), Some(22));
    /// ```
    pub const CONTINUE: Self = ControlFlow::Continue(());
}

impl<C> ControlFlow<(), C> {
    /// APIs like `try_for_each` don't need values with `Break`,
    /// so this provides a way to avoid typing `(())`, if you prefer it.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(control_flow_enum)]
    /// use std::ops::ControlFlow;
    ///
    /// let mut partial_sum = 0;
    /// (1..10).chain(20..25).try_for_each(|x| {
    ///     if partial_sum > 100 { ControlFlow::BREAK }
    ///     else { partial_sum += x; ControlFlow::CONTINUE }
    /// });
    /// assert_eq!(partial_sum, 108);
    /// ```
    pub const BREAK: Self = ControlFlow::Break(());
}
