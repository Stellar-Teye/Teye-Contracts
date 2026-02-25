# RBAC Quick Reference

## Role to Base Permissions (get_base_permissions)

| Role | Level | Base Permissions |
|------|-------|------------------|
| Admin | 5 | SystemAdmin, ManageUsers, ManageAccess, WriteRecord, ReadAnyRecord |
| Ophthalmologist | 4 | ManageUsers, ManageAccess, WriteRecord, ReadAnyRecord |
| Optometrist | 3 | ManageUsers, ManageAccess, WriteRecord, ReadAnyRecord |
| Staff | 2 | ManageUsers |
| Patient | 1 | None (implicit own-record access outside RBAC) |
| None | 0 | None |

## Permission to Roles (defaults)

| Permission | Default Roles |
|------------|---------------|
| ReadAnyRecord | Admin, Ophthalmologist, Optometrist |
| WriteRecord | Admin, Ophthalmologist, Optometrist |
| ManageAccess | Admin, Ophthalmologist, Optometrist |
| ManageUsers | Admin, Ophthalmologist, Optometrist, Staff |
| SystemAdmin | Admin |

## Notes

- This table reflects only base permissions from `get_base_permissions`.
- Custom grants/revokes, ACL groups, and delegations are evaluated separately.
