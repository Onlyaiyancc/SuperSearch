# Everything SDK provenance

Source: [official Everything SDK](https://www.voidtools.com/Everything-SDK.zip), downloaded 2026-10-12.

Original ZIP SHA256: `F5716D9513CCE6B462B5170A0A2E7E081E191D9BCAC6774F5DECC061497DF443`.

The SDK C source and headers include David Carpenter's MIT license. All copyright and license notices are retained. Two relative includes in Everything.c were normalized for this directory layout. `bridge.c` bounds SDK SendMessage calls, validates reply payloads and scopes IPC to the private instance. The SDK is statically linked. Since SuperSearch 0.1.4, the unmodified official Everything executable and its full vendor/PCRE notices are embedded separately; provenance is recorded in `../resources/everything/README.md`. The index remains a separate ordinary-user background process, not an SDK library implementation.
