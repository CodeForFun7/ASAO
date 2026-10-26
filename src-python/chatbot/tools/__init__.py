import json
from urllib.error import URLError
from urllib.request import Request, urlopen

from langchain_core.tools import tool


@tool
def get_info() -> str:
    """Get current system metrics from the Rust system monitor."""
    request = Request("http://127.0.0.1:8766/get_info", method="GET")
    try:
        with urlopen(request, timeout=8) as response:
            return json.dumps(json.loads(response.read().decode("utf-8")))
    except (URLError, TimeoutError, ValueError) as exc:
        return f"System metrics are unavailable: {exc}"


TOOLS = [get_info]
