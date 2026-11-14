import json

import playwright_rust
from playwright_rust import _native

print(
    json.dumps(
        {
            "abi": _native.native_abi_version(),
            "capabilities": playwright_rust.native_capabilities(),
            "module": _native.__file__,
        },
        sort_keys=True,
    )
)
