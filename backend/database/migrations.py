"""Schema versioning for SQLite database.

For the MVP, tables are created via SQLAlchemy's create_all.
This module provides a framework for future migrations.
"""

from sqlalchemy import text
from sqlalchemy.ext.asyncio import AsyncSession

SCHEMA_VERSION = 2


async def get_schema_version(session: AsyncSession) -> int:
    """Read current schema version from system_state."""
    try:
        result = await session.execute(
            text("SELECT value FROM system_state WHERE key = 'schema_version'")
        )
        row = result.first()
        return int(row[0]) if row else 0
    except Exception:
        return 0


async def set_schema_version(session: AsyncSession, version: int):
    """Write schema version to system_state."""
    await session.execute(
        text(
            "INSERT OR REPLACE INTO system_state (key, value, updated_at) "
            "VALUES ('schema_version', :version, CURRENT_TIMESTAMP)"
        ),
        {"version": str(version)},
    )
    await session.commit()


async def _migrate_v1_to_v2(session: AsyncSession):
    """Add profiles table and profile_id columns to products and campaigns."""
    # The profiles table is created by create_all in init_db().
    # Here we add the profile_id column to existing tables if missing.
    # SQLite requires checking pragma table_info to see if column exists.
    for table_name in ("products", "campaigns"):
        result = await session.execute(text(f"PRAGMA table_info({table_name})"))
        columns = [row[1] for row in result.all()]
        if "profile_id" not in columns:
            await session.execute(
                text(f"ALTER TABLE {table_name} ADD COLUMN profile_id TEXT REFERENCES profiles(id)")
            )
    await session.commit()


async def run_migrations(session: AsyncSession):
    """Run any pending migrations."""
    current = await get_schema_version(session)
    if current < SCHEMA_VERSION:
        if current < 2:
            await _migrate_v1_to_v2(session)
        await set_schema_version(session, SCHEMA_VERSION)
