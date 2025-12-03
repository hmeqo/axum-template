use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create users table
        manager
            .create_table(
                Table::create()
                    .table(User::Table)
                    .if_not_exists()
                    .col(pk_auto(User::Id))
                    .col(string(User::Username))
                    .col(string(User::Password))
                    .col(
                        timestamp_with_time_zone(User::CreatedAt)
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp_with_time_zone(User::UpdatedAt)
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create roles table
        manager
            .create_table(
                Table::create()
                    .table(Role::Table)
                    .if_not_exists()
                    .col(pk_auto(Role::Id))
                    .col(string_uniq(Role::Name))
                    .col(string_null(Role::Description))
                    .col(
                        timestamp_with_time_zone(Role::CreatedAt)
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp_with_time_zone(Role::UpdatedAt)
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create permissions table
        manager
            .create_table(
                Table::create()
                    .table(Permission::Table)
                    .if_not_exists()
                    .col(pk_auto(Permission::Id))
                    .col(string(Permission::Resource))
                    .col(string(Permission::Action))
                    .col(string_null(Permission::Description))
                    .col(
                        timestamp_with_time_zone(Permission::CreatedAt)
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create unique index for permissions
        manager
            .create_index(
                Index::create()
                    .name("idx_permissions_resource_action")
                    .table(Permission::Table)
                    .col(Permission::Resource)
                    .col(Permission::Action)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Create role_permissions junction table
        manager
            .create_table(
                Table::create()
                    .table(RolePermission::Table)
                    .if_not_exists()
                    .col(pk_auto(RolePermission::Id))
                    .col(integer(RolePermission::RoleId))
                    .col(integer(RolePermission::PermissionId))
                    .col(
                        timestamp_with_time_zone(RolePermission::CreatedAt)
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_role_permission_role")
                            .from(RolePermission::Table, RolePermission::RoleId)
                            .to(Role::Table, Role::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_role_permission_permission")
                            .from(RolePermission::Table, RolePermission::PermissionId)
                            .to(Permission::Table, Permission::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create unique index for role_permissions
        manager
            .create_index(
                Index::create()
                    .name("idx_role_permissions_unique")
                    .table(RolePermission::Table)
                    .col(RolePermission::RoleId)
                    .col(RolePermission::PermissionId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Create user_roles junction table
        manager
            .create_table(
                Table::create()
                    .table(UserRole::Table)
                    .if_not_exists()
                    .col(pk_auto(UserRole::Id))
                    .col(integer(UserRole::UserId))
                    .col(integer(UserRole::RoleId))
                    .col(
                        timestamp_with_time_zone(UserRole::CreatedAt)
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_role_user")
                            .from(UserRole::Table, UserRole::UserId)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_role_role")
                            .from(UserRole::Table, UserRole::RoleId)
                            .to(Role::Table, Role::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create unique index for user_roles
        manager
            .create_index(
                Index::create()
                    .name("idx_user_roles_unique")
                    .table(UserRole::Table)
                    .col(UserRole::UserId)
                    .col(UserRole::RoleId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserRole::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(RolePermission::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Permission::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Role::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(User::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum User {
    Table,
    Id,
    Username,
    Password,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Role {
    Table,
    Id,
    Name,
    Description,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Permission {
    Table,
    Id,
    Resource,
    Action,
    Description,
    CreatedAt,
}

#[derive(DeriveIden)]
enum RolePermission {
    Table,
    Id,
    RoleId,
    PermissionId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum UserRole {
    Table,
    Id,
    UserId,
    RoleId,
    CreatedAt,
}
