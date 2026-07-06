use anyhow::Context;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Insert};

pub const DEFAULT_BATCH_SIZE: usize = 500;

/// Inserts models in batches, safely handling empty iterators.
///
/// `configure` is called on each batch's `Insert` to apply customizations
/// such as `.on_conflict()`.
pub async fn batch_insert<A, I, F>(
    db: &DatabaseConnection,
    models: I,
    batch_size: usize,
    configure: F,
) -> anyhow::Result<()>
where
    A: ActiveModelTrait + Send,
    I: IntoIterator<Item = A>,
    F: Fn(Insert<A>) -> Insert<A>,
{
    let mut iter = models.into_iter().peekable();
    while iter.peek().is_some() {
        configure(<A::Entity as EntityTrait>::insert_many(
            (&mut iter).take(batch_size),
        ))
        .exec(db)
        .await
        .context("batch insert failed")?;
    }
    Ok(())
}
