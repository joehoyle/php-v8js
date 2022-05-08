<?php

$v8js = new V8Js;

try {
    $result = $v8js->executeString( 'my_func();' );
} catch ( V8JsScriptException $e ) {
    assert( $e->getJsFileName() );
}



