<?php

$v8js = new V8Js;
$v8js->sleep = function ( int $milliseconds ) {
    usleep( $milliseconds * 1000 );
};

$killed = null;
$used_memory_limit_exception = false;
try {
    $result = $v8js->executeString( 'let my_arr = []; for ( let i = 0; i < 100000 ; i++ ) { my_arr.push( (new Date).toString() ); }; ', null, null, null, 1024 * 10 );
    $killed = false;
} catch ( V8JsMemoryLimitException $e ) {
    $killed = true;
    $used_memory_limit_exception = true;
} catch ( Exception $e ) {
    $killed = true;
}
assert( $killed === true );
assert( $used_memory_limit_exception === true );
