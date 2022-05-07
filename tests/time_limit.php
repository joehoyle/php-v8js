<?php

$v8js = new V8Js;
$v8js->sleep = function ( int $milliseconds ) {
    usleep( $milliseconds * 1000 );
};

$killed = null;
$used_time_limit_exception = false;
try {
    $result = $v8js->executeString( 'for ( let i = 0; i < 100 ; i++ ) { PHP.sleep(100); }; "done"', null, null, 100 );
    $killed = false;
} catch( V8JsTimeLimitException ) {
    $killed = true;
    $used_time_limit_exception = true;
} catch ( Exception $e ) {
    $killed = true;
}
assert( $killed === true );
assert( $used_time_limit_exception === true );
