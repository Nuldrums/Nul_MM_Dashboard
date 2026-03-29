"""CRUD endpoints for products."""

from uuid import uuid4
from datetime import datetime
from typing import Optional
from fastapi import APIRouter, Depends, HTTPException, Query
from pydantic import BaseModel
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from backend.database.connection import get_db
from backend.database.models import Product

router = APIRouter(prefix="/api/products", tags=["products"])


class ProductCreate(BaseModel):
    name: str
    type: str
    description: str | None = None
    url: str | None = None
    price: float | None = None
    tags: str | None = None  # JSON array string
    profile_id: str | None = None


class ProductUpdate(BaseModel):
    name: str | None = None
    type: str | None = None
    description: str | None = None
    url: str | None = None
    price: float | None = None
    tags: str | None = None
    profile_id: str | None = None


class ProductResponse(BaseModel):
    id: str
    name: str
    type: str
    description: str | None = None
    url: str | None = None
    price: float | None = None
    tags: str | None = None
    profile_id: str | None = None
    created_at: datetime | None = None

    model_config = {"from_attributes": True}


@router.get("", response_model=list[ProductResponse])
async def list_products(
    profile_id: Optional[str] = Query(None),
    db: AsyncSession = Depends(get_db),
):
    query = select(Product)
    if profile_id is not None:
        query = query.where(Product.profile_id == profile_id)
    result = await db.execute(query.order_by(Product.created_at.desc()))
    return result.scalars().all()


@router.post("", response_model=ProductResponse, status_code=201)
async def create_product(data: ProductCreate, db: AsyncSession = Depends(get_db)):
    product = Product(id=str(uuid4()), **data.model_dump())
    db.add(product)
    await db.commit()
    await db.refresh(product)
    return product


@router.put("/{product_id}", response_model=ProductResponse)
async def update_product(
    product_id: str, data: ProductUpdate, db: AsyncSession = Depends(get_db)
):
    result = await db.execute(select(Product).where(Product.id == product_id))
    product = result.scalar_one_or_none()
    if not product:
        raise HTTPException(status_code=404, detail="Product not found")
    for key, value in data.model_dump(exclude_unset=True).items():
        setattr(product, key, value)
    await db.commit()
    await db.refresh(product)
    return product


@router.delete("/{product_id}", status_code=204)
async def delete_product(product_id: str, db: AsyncSession = Depends(get_db)):
    result = await db.execute(select(Product).where(Product.id == product_id))
    product = result.scalar_one_or_none()
    if not product:
        raise HTTPException(status_code=404, detail="Product not found")
    await db.delete(product)
    await db.commit()
