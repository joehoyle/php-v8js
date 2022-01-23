<?php

$v8js = new V8Js;
$v8js->sleep = function ( int $milliseconds ) {
    usleep( $milliseconds * 1000 );
};

$killed = null;
try {
    $result = $v8js->executeString( 'for ( let i = 0; i < 100 ; i++ ) { PHP.sleep(100); }; "done"', null, null, 100 );
    $killed = false;
} catch ( Exception $e ) {
    // Todo: specific exceptions.
    $killed = true;
}
assert( $killed === true );

$v8js = new V8Js;
$v8js->sleep = function ( int $milliseconds ) {
    usleep( $milliseconds * 1000 );
};

var_dump("on to the next");

$killed = null;
try {
    $result = $v8js->executeString( 'let my_arr = []; for ( let i = 0; i < 100000 ; i++ ) { my_arr.push( (new Date).toString() ); };', null, null, null, null );
    $killed = false;
} catch ( Exception $e ) {
    // Todo: specific exceptions.
    $killed = true;
}
assert( $killed === true );
