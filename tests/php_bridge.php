<?php

$v8js = new V8Js;
$result = $v8js->executeString( 'PHP' );
assert( $result === [] );

$v8js = new V8Js('MyBridge');
$result = $v8js->executeString( 'MyBridge' );
assert( $result === [] );

$v8js = new V8Js;
$v8js->my_var = 'abc';
$result = $v8js->executeString( 'PHP.my_var' );
assert( $result === 'abc' );

$v8js = new V8Js;
$v8js->my_func = function () {
    return 1;
};
$result = $v8js->executeString( 'PHP.my_func()' );
assert( $result === 1 );

$v8js = new V8Js;
$v8js->sleep = function ( int $milliseconds ) {
    usleep( $milliseconds * 1000 );
    return "done";
};
$result = $v8js->executeString( 'PHP.sleep(200);' );
assert( $result === "done" );
