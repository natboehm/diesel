use expression::SelectableExpression;
use expression::grouped::Grouped;
use expression::nullable::Nullable;
use prelude::*;
use query_builder::*;
use result::QueryResult;
use super::QuerySource;
use util::TupleAppend;

#[derive(Debug, Clone, Copy)]
/// A query source representing the join between two tables
pub struct Join<Left, Right, Kind> {
    left: Left,
    right: Right,
    kind: Kind,
}

#[derive(Debug, Clone, Copy)]
#[doc(hidden)]
/// A query source representing the join between two tables with an explicit
/// `ON` given. `Join` should usually be referenced instead, as all "type
/// safety" traits are implemented in terms of `Join` implementing them.
pub struct JoinOn<Join, On> {
    join: Join,
    on: On,
}

impl<Left, Right, Kind> Join<Left, Right, Kind> {
    pub fn new(left: Left, right: Right, kind: Kind) -> Self {
        Join {
            left: left,
            right: right,
            kind: kind,
        }
    }

    #[doc(hidden)]
    pub fn on<On>(self, on: On) -> JoinOn<Self, On> {
        JoinOn {
            join: self,
            on: on,
        }
    }
}

impl_query_id!(Join<Left, Right, Kind>);
impl_query_id!(JoinOn<Join, On>);

impl<Left, Right> QuerySource for Join<Left, Right, Inner> where
    Left: QuerySource + JoinTo<Right> + AppendSelection<Right::DefaultSelection>,
    Right: QuerySource,
    Left::Output: SelectableExpression<Self>,
    Self: Clone,
{
    type FromClause = Self;
    type DefaultSelection = Left::Output;

    fn from_clause(&self) -> Self::FromClause {
        self.clone()
    }

    fn default_selection(&self) -> Self::DefaultSelection {
        self.left.append_selection(self.right.default_selection())
    }
}

impl<Left, Right> QuerySource for Join<Left, Right, LeftOuter> where
    Left: QuerySource + JoinTo<Right> + AppendSelection<Nullable<Right::DefaultSelection>>,
    Right: QuerySource,
    Left::Output: SelectableExpression<Self>,
    Self: Clone,
{
    type FromClause = Self;
    type DefaultSelection = Left::Output;

    fn from_clause(&self) -> Self::FromClause {
        self.clone()
    }

    fn default_selection(&self) -> Self::DefaultSelection {
        self.left.append_selection(self.right.default_selection().nullable())
    }
}

impl<Join, On> QuerySource for JoinOn<Join, On> where
    Join: QuerySource,
    On: AppearsOnTable<Join::FromClause> + Clone,
    Join::DefaultSelection: SelectableExpression<Self>,
{
    type FromClause = Grouped<nodes::InfixNode<'static, Join::FromClause, On>>;
    type DefaultSelection = Join::DefaultSelection;

    fn from_clause(&self) -> Self::FromClause {
        Grouped(nodes::InfixNode::new(
            self.join.from_clause(),
            self.on.clone(),
            " ON ",
        ))
    }

    fn default_selection(&self) -> Self::DefaultSelection {
        self.join.default_selection()
    }
}

impl<Left, Right, Kind, DB> QueryFragment<DB> for Join<Left, Right, Kind> where
    DB: Backend,
    Left: QuerySource,
    Left::FromClause: QueryFragment<DB>,
    Right: QuerySource,
    Right::FromClause: QueryFragment<DB>,
    Kind: QueryFragment<DB>,
{
    fn walk_ast(&self, mut out: AstPass<DB>) -> QueryResult<()> {
        self.left.from_clause().walk_ast(out.reborrow())?;
        self.kind.walk_ast(out.reborrow())?;
        out.push_sql(" JOIN ");
        self.right.from_clause().walk_ast(out.reborrow())?;
        Ok(())
    }
}

impl<Left, Right, Kind, T> SelectableExpression<Join<Left, Right, Kind>>
    for Nullable<T> where
        T: SelectableExpression<Join<Left, Right, Inner>>,
        Nullable<T>: AppearsOnTable<Join<Left, Right, Kind>>,
{
}

// FIXME: Remove this when overlapping marker traits are stable
impl<Join, On, T> SelectableExpression<JoinOn<Join, On>>
    for Nullable<T> where
        Nullable<T>: SelectableExpression<Join>,
        Nullable<T>: AppearsOnTable<JoinOn<Join, On>>,
{
}

// FIXME: Remove this when overlapping marker traits are stable
impl<From, T> SelectableExpression<SelectStatement<From>>
    for Nullable<T> where
        Nullable<T>: SelectableExpression<From>,
        Nullable<T>: AppearsOnTable<SelectStatement<From>>,
{
}

// FIXME: We want these blanket impls when overlapping marker traits are stable
// impl<T, Join, On> SelectableExpression<JoinOn<Join, On>> for T where
//     T: SelectableExpression<Join> + AppearsOnTable<JoinOn<Join, On>>,
// {
// }

/// Indicates that two tables can be used together in a JOIN clause.
/// Implementations of this trait will be generated for you automatically by
/// the [association annotations](../associations/index.html) from codegen.
pub trait JoinTo<T> {
    #[doc(hidden)]
    type JoinOnClause;
    #[doc(hidden)]
    fn join_on_clause() -> Self::JoinOnClause;
}

#[doc(hidden)]
/// Used to ensure the sql type of `left.join(mid).join(right)` is
/// `(Left, Mid, Right)` and not `((Left, Mid), Right)`. This needs
/// to be separate from `TupleAppend` because we still want to keep
/// the column lists (which are tuples) separate.
pub trait AppendSelection<Selection> {
    type Output;

    fn append_selection(&self, selection: Selection) -> Self::Output;
}

impl<T: Table, Selection> AppendSelection<Selection> for T {
    type Output = (T::AllColumns, Selection);

    fn append_selection(&self, selection: Selection) -> Self::Output {
        (T::all_columns(), selection)
    }
}

impl<Left, Mid, Selection, Kind> AppendSelection<Selection> for Join<Left, Mid, Kind> where
    Self: QuerySource,
    <Self as QuerySource>::DefaultSelection: TupleAppend<Selection>,
{
    type Output = <<Self as QuerySource>::DefaultSelection as TupleAppend<Selection>>::Output;

    fn append_selection(&self, selection: Selection) -> Self::Output {
        self.default_selection().tuple_append(selection)
    }
}

impl<Join, On, Selection> AppendSelection<Selection> for JoinOn<Join, On> where
    Join: AppendSelection<Selection>,
{
    type Output = Join::Output;

    fn append_selection(&self, selection: Selection) -> Self::Output {
        self.join.append_selection(selection)
    }
}

use backend::Backend;

#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Inner;
impl_query_id!(Inner);

impl<DB: Backend> QueryFragment<DB> for Inner {
    fn walk_ast(&self, mut out: AstPass<DB>) -> QueryResult<()> {
        out.push_sql(" INNER");
        Ok(())
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default)]
pub struct LeftOuter;
impl_query_id!(LeftOuter);

impl<DB: Backend> QueryFragment<DB> for LeftOuter {
    fn walk_ast(&self, mut out: AstPass<DB>) -> QueryResult<()> {
        out.push_sql(" LEFT OUTER");
        Ok(())
    }
}

impl<Left, Mid, Right, Kind> JoinTo<Right> for Join<Left, Mid, Kind> where
    Left: JoinTo<Right>,
{
    type JoinOnClause = Left::JoinOnClause;

    fn join_on_clause() -> Self::JoinOnClause {
        Left::join_on_clause()
    }
}

impl<Join, On, Right> JoinTo<Right> for JoinOn<Join, On> where
    Join: JoinTo<Right>,
{
    type JoinOnClause = Join::JoinOnClause;

    fn join_on_clause() -> Self::JoinOnClause {
        Join::join_on_clause()
    }
}

use super::{Succ, Never, AppearsInFromClause};

impl<T, Left, Right, Kind> AppearsInFromClause<T> for Join<Left, Right, Kind> where
    Left: AppearsInFromClause<T>,
    Right: AppearsInFromClause<T>,
    Left::Count: Plus<Right::Count>,
{
    type Count = <Left::Count as Plus<Right::Count>>::Output;
}

impl<T, Join, On> AppearsInFromClause<T> for JoinOn<Join, On> where
    Join: AppearsInFromClause<T>,
{
    type Count = Join::Count;
}

#[allow(missing_debug_implementations, missing_copy_implementations)]
#[doc(hidden)]
/// A hack to allow bidirectional joins to be generated from `#[belongs_to]`
///
/// This type needs to exist because it is illegal in Rust today to write
/// `impl JoinTo<posts> for <User as HasTable>::Table`, even though the type
/// is fully monomorphic and projects to a local type. If this restriction
/// were ever lifted in the future, this type could be removed.
///
/// Instead, after generating `impl JoinTo<<User as HasTable>::Table> for
/// posts`, we *also* generate `impl JoinTo<PleaseGenerateInverseJoinImpls<<User
/// as HasTable>::table> for posts`, and rely on the fact that `users::table`
/// will have a blanket impl on itself for anything that joins to
/// `PleaseGenerateInverseJoinImpls`.
pub struct PleaseGenerateInverseJoinImpls<T>(T);

pub trait Plus<T> {
    type Output;
}

impl<T, U> Plus<T> for Succ<U> where
    U: Plus<T>,
{
    type Output = Succ<U::Output>;
}

impl<T> Plus<T> for Never {
    type Output = T;
}
