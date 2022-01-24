# PHP-V8Js

PHP-V8Js is a PHP extension for the V8 JavaScript engine. It is a re-implementation of the fantastic (though unmaintained) [V8Js PHP extension](https://github.com/phpv8/v8js).

The extension allows you to execute JavaScript code in a secure sandbox from PHP. The executed code can be restricted using a time limit and/or memory limit. This provides the possibility to execute untrusted code with confidence.

## Requirements

- PHP 8.0+

The extension includes builds of libv8, via the [v8 crate](https://docs.rs/v8/latest/v8/). This makes installing the extension very simple.

## Todo:

### V8Js Compatibility

- [x] Memory / time limits
- [x] Snapshop creating and loading
- [x] Default global functions `var_dump`, `sleep`, `exit`
- [ ] Default global function `print`
- [ ] CommonJS / `require` support
- [ ] `setModuleLoader`
- [ ] `setModuleNormalizer`
- [ ] Subclassing V8Js
- [ ] Custom exceptions for `V8JsScriptException`, `V8JsMemoryLimitException` and `V8JsTimeLimitException`
- [ ] Support for `FLAG_PROPAGATE_PHP_EXCEPTIONS`
- [ ] Throw correct exception subclasses

### Not planned compatibility

- [ ] Support for `ArrayAccess` objects mapped into JavaScript

### New features

- [ ] Support for native ES modules

## Credits

- [Stefan Siegl](https://github.com/stesie) of course for creating v8js, and the [37 contributors](https://github.com/phpv8/v8js/graphs/contributors).
- [David Cole](https://github.com/davidcole1340) for creating ext-php-rs and helping me use the library.
