import logging

from fastapi_cache import FastAPICache

logger = logging.getLogger(__name__)


async def clear_namespace(namespace: str) -> None:
    """Clear all Redis cache keys for a given namespace."""
    if not namespace:
        raise ValueError("namespace must be a non-empty string")
    try:
        await FastAPICache.clear(namespace=namespace)
    except Exception:
        logger.exception("Failed to clear cache namespace: %s", namespace)
