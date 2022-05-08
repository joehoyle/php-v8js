# PHP-V8Js

PHP-V8Js is a PHP extension for the V8 JavaScript engine. It is a re-implementation of the fantastic (though unmaintained) [V8Js PHP extension](https://github.com/phpv8/v8js).

The extension allows you to execute JavaScript code in a secure sandbox from PHP. The executed code can be restricted using a time limit and/or memory limit. This provides the possibility to execute untrusted code with confidence.

## Requirements

- PHP 8.0+

The extension includes builds of libv8, via the [v8 crate](https://docs.rs/v8/latest/v8/). This makes installing the extension very simple.

## Mapping Rules

|PHP -> JavaScript|JavaScript -> PHP|
|---|---|
|String -> string|string -> String|
|Bool -> bool|bool -> Bool|
|Array (numeric) -> Array|array -> Array|
|Array (string keys) -> Object|Object -> V8Object|
|Int -> Number|Number -> Float|


## Todo:

### V8Js Compatibility

- [x] Memory / time limits
- [x] Snapshop creating and loading
- [x] Default global functions `var_dump`, `sleep`, `exit`
- [ ] Default global function `print`
- [x] CommonJS / `require` support
- [x] `setModuleLoader`
- [ ] `setModuleNormaliser`
- [ ] Subclassing V8Js
- [x] Custom exceptions for `V8JsScriptException`, `V8JsMemoryLimitException` and `V8JsTimeLimitException`
- [ ] Support for `V8JsScriptException::getJsLineNumber` etc.
- [ ] Support for `FLAG_PROPAGATE_PHP_EXCEPTIONS`, `V8Js::FLAG_FORCE_ARRAY`
- [ ] Throw correct exception subclasses
- [ ] PHP INI settings `v8js.flags`
- [x] `V8Js::V8_VERSION` constant

### Not planned compatibility

- Support for `ArrayAccess` objects mapped into JavaScript
- PHP INI settings `v8js.use_array_access`, `v8js.use_date`, `v8js.icudtl_dat_path`
- `V8Js::registerExtension`

### New features

- [ ] Support for native ES modules

## Credits

- [Stefan Siegl](https://github.com/stesie) of course for creating v8js, and the [37 contributors](https://github.com/phpv8/v8js/graphs/contributors).
- [David Cole](https://github.com/davidcole1340) for creating ext-php-rs and helping me use the library.
