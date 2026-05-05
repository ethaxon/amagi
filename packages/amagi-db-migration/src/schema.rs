use sea_orm_migration::{
    prelude::*,
    schema::*,
    sea_query::{ColumnDef, ColumnType, Expr, IndexOrder, IndexType, IntoIden, SeaRc},
};

pub async fn create_postgres_auto_update_ts_fn(
    manager: &SchemaManager<'_>,
    col_name: &str,
) -> Result<(), DbErr> {
    let sql = format!(
        "CREATE OR REPLACE FUNCTION update_{col_name}_column() RETURNS TRIGGER AS $$ BEGIN \
         NEW.{col_name} = current_timestamp; RETURN NEW; END; $$ language 'plpgsql';"
    );

    manager.get_connection().execute_unprepared(&sql).await?;

    Ok(())
}

pub async fn create_postgres_auto_update_ts_trigger(
    manager: &SchemaManager<'_>,
    col_name: &str,
    table_name: &str,
) -> Result<(), DbErr> {
    let sql = format!(
        "CREATE OR REPLACE TRIGGER update_{table_name}_{col_name}_column_trigger BEFORE UPDATE ON \
         {table_name} FOR EACH ROW EXECUTE PROCEDURE update_{col_name}_column();"
    );

    manager.get_connection().execute_unprepared(&sql).await?;

    Ok(())
}

pub async fn drop_postgres_auto_update_ts_trigger(
    manager: &SchemaManager<'_>,
    col_name: &str,
    table_name: &str,
) -> Result<(), DbErr> {
    let sql = format!(
        "DROP TRIGGER IF EXISTS update_{table_name}_{col_name}_column_trigger ON {table_name};"
    );

    manager.get_connection().execute_unprepared(&sql).await?;

    Ok(())
}

pub async fn drop_postgres_auto_update_ts_fn(
    manager: &SchemaManager<'_>,
    col_name: &str,
) -> Result<(), DbErr> {
    let sql = format!("DROP FUNCTION IF EXISTS update_{col_name}_column();");

    manager.get_connection().execute_unprepared(&sql).await?;

    Ok(())
}

pub fn pk_uuid_v7<T: IntoIden>(name: T) -> ColumnDef {
    uuid(name)
        .default(Expr::cust("uuidv7()"))
        .primary_key()
        .take()
}

pub fn shared_pk_uuid<T: IntoIden>(name: T) -> ColumnDef {
    pk_uuid(name)
}

pub fn timestamptz<T: IntoIden>(col: T) -> ColumnDef {
    timestamp_with_time_zone(col)
        .default(Expr::current_timestamp())
        .take()
}

pub fn timestamptz_null<T: IntoIden>(col: T) -> ColumnDef {
    timestamp_with_time_zone_null(col)
}

pub fn boolean_default_false<T: IntoIden>(col: T) -> ColumnDef {
    boolean(col).default(false).take()
}

pub fn boolean_default_true<T: IntoIden>(col: T) -> ColumnDef {
    boolean(col).default(true).take()
}

pub fn jsonb<T: IntoIden>(col: T) -> ColumnDef {
    json_binary(col)
}

pub fn jsonb_default_object<T: IntoIden>(col: T) -> ColumnDef {
    json_binary(col).default(Expr::cust("'{}'::jsonb")).take()
}

pub fn jsonb_default_array<T: IntoIden>(col: T) -> ColumnDef {
    json_binary(col).default(Expr::cust("'[]'::jsonb")).take()
}

pub fn text_array_default_empty<T: IntoIden>(col: T) -> ColumnDef {
    array(col, ColumnType::Text)
        .default(Expr::cust("'{}'::text[]"))
        .take()
}

pub fn index<T: IntoIden, C: IntoIden + Copy>(
    name: &str,
    table: T,
    columns: &[C],
) -> IndexCreateStatement {
    let mut statement = Index::create();
    statement.name(name).table(table).if_not_exists();
    for column in columns {
        statement.col(*column);
    }
    statement.to_owned()
}

pub fn unique_index<T: IntoIden, C: IntoIden + Copy>(
    name: &str,
    table: T,
    columns: &[C],
) -> IndexCreateStatement {
    let mut statement = index(name, table, columns);
    statement.unique();
    statement
}

pub fn gin_index<T: IntoIden, C: IntoIden + Copy>(
    name: &str,
    table: T,
    columns: &[C],
) -> IndexCreateStatement {
    let mut statement = index(name, table, columns);
    statement.index_type(IndexType::Custom(SeaRc::new("gin")));
    statement
}

pub fn desc_index<T: IntoIden, C: IntoIden + Copy>(
    name: &str,
    table: T,
    first_col: C,
    desc_col: C,
) -> IndexCreateStatement {
    Index::create()
        .name(name)
        .table(table)
        .if_not_exists()
        .col(first_col)
        .col((Expr::col(desc_col), IndexOrder::Desc))
        .to_owned()
}

#[cfg(test)]
mod tests {
    use sea_orm_migration::sea_query::PostgresQueryBuilder;

    use super::*;

    #[derive(DeriveIden, Clone, Copy)]
    enum ExampleTable {
        Table,
        Id,
        Tags,
        CreatedAt,
        Name,
    }

    #[test]
    fn schema_helpers_render_uuidv7_and_defaults() {
        let sql = Table::create()
            .table(ExampleTable::Table)
            .col(pk_uuid_v7(ExampleTable::Id))
            .col(jsonb_default_object(ExampleTable::Tags))
            .col(timestamptz(ExampleTable::CreatedAt))
            .to_string(PostgresQueryBuilder);

        let array_sql = Table::create()
            .table(ExampleTable::Table)
            .col(jsonb_default_array(ExampleTable::Tags))
            .to_string(PostgresQueryBuilder);

        assert!(sql.contains("uuidv7()"));
        assert!(sql.contains("'{}'::jsonb"));
        assert!(sql.contains("CURRENT_TIMESTAMP"));
        assert!(array_sql.contains("'[]'::jsonb"));
    }

    #[test]
    fn index_helpers_render_expected_postgres_sql() {
        let unique_sql = unique_index(
            "idx_example_name_unique",
            ExampleTable::Table,
            &[ExampleTable::Name],
        )
        .to_string(PostgresQueryBuilder);
        let gin_sql = gin_index(
            "idx_example_tags_gin",
            ExampleTable::Table,
            &[ExampleTable::Tags],
        )
        .to_string(PostgresQueryBuilder);

        assert!(unique_sql.contains("CREATE UNIQUE INDEX"));
        assert!(gin_sql.contains("CREATE INDEX"));
        assert!(gin_sql.to_ascii_lowercase().contains("using gin"));
    }
}
