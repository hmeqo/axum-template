use sea_orm_migration::{
    prelude::{ColumnDef, IntoIden},
    schema::pk_auto,
};

pub fn pk_auto_bigint<T: IntoIden>(column: T) -> ColumnDef {
    pk_auto(column).big_integer().take()
}
