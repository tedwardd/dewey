# Changelog

## 0.1.0 (2026-08-16)


### Features

* add Book/Category data model and format tags ([e09b9e4](https://github.com/tedwardd/dewey/commit/e09b9e45b7f19c9e80387ad7d8bb747c664b7cd1))
* add config loading and module path resolution ([f8c2753](https://github.com/tedwardd/dewey/commit/f8c2753302199e9914a93e63ad0dbd28030b1aaf))
* add gutenberg module with fixtures and tests ([9ff1927](https://github.com/tedwardd/dewey/commit/9ff19271ad7efd95a50a6e417dbf7e26730bbc21))
* add host-owned downloader with retry and naming ([2cea18e](https://github.com/tedwardd/dewey/commit/2cea18e3b857f85d71e2e7c0ddc8a18875de5bbd))
* add JSON-RPC request/response framing ([aa61089](https://github.com/tedwardd/dewey/commit/aa610895e4185df8d1fbd514e9e7bee83fef2607))
* add module discovery and manifest validation ([e26a5f0](https://github.com/tedwardd/dewey/commit/e26a5f0f219a98d02f6e4f7558645b46b84b5554))
* add module host with one-shot exchange and timeout ([5da917c](https://github.com/tedwardd/dewey/commit/5da917ce97cc127e1d1fbd8b0c6eb0a25a8c5b74))
* add standard-ebooks module with fixtures and tests ([bad85e6](https://github.com/tedwardd/dewey/commit/bad85e6e88c4ef9969f55293dee1544566d4cfc5))
* add table and JSON output rendering ([cbbb403](https://github.com/tedwardd/dewey/commit/cbbb4036367de03e9e27ec6e06c7adede56a51df))
* end-to-end download proof, live tests, README ([47c41b6](https://github.com/tedwardd/dewey/commit/47c41b68085e3746b73ce0978c5ef3e807dc5527))
* rename modules verb to libraries ([7f6b20c](https://github.com/tedwardd/dewey/commit/7f6b20cf72ec4e8dd66b09ac9b758c9c75d17cdc))
* verify install verb copies and guards overwrites ([84a8b30](https://github.com/tedwardd/dewey/commit/84a8b3057848ab3090fd99e256c89983ce423ef5))
* wire CLI verbs, module resolution, and exit codes ([19ab916](https://github.com/tedwardd/dewey/commit/19ab91621db1fd02926c480e8e71bc1bdbf565f6))


### Bug Fixes

* gate verbs by capability, add download timeout, harden module errors ([f780265](https://github.com/tedwardd/dewey/commit/f780265fc497764cfe25d06d1c1138de54591331))
* isolate test HOME, clean install target on force, test help/version ([c9071fe](https://github.com/tedwardd/dewey/commit/c9071fe7e393f1262209b2f58fc9faa3031849a2))
* kill module before joining reader thread on timeout ([474fce6](https://github.com/tedwardd/dewey/commit/474fce61f0cede75595d64d913aeca8f7e25cdb1))
* retry mid-body failures, sanitize author, guard empty title ([6f1e46a](https://github.com/tedwardd/dewey/commit/6f1e46a9fbd5a3db57293aa352b6fcd88a69f5c5))
* standard-ebooks fixture errors as -32000, cover html fallback ([dddbc5c](https://github.com/tedwardd/dewey/commit/dddbc5c90adc10da8a7c9dcc16512f73a4a08b74))
