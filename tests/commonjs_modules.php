<?php

$v8js = new V8Js;
$v8js->setModuleLoader( function ( string $module ) : string {
    return 'module.exports = 10';
} );

assert( 10 === $v8js->executeString( 'require("lodash")') );

$v8js = new V8Js;
$v8js->setModuleLoader( function ( string $module ) : string {
    return 'exports.get = 10';
} );

assert( 10 === $v8js->executeString( 'require("lodash").get') );
