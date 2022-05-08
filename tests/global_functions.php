<?php

// Todo: print needs implementing.

$v8js = new V8Js;
ob_start();
$v8js->executeString( 'print("hello")' );
$result = ob_get_clean();
assert( $result === "hello");

$v8js = new V8Js;
ob_start();
$v8js->executeString( 'var_dump("hello")' );
$result = ob_get_clean();
assert( $result === 'string(5) "hello"' . "\n");

$v8js = new V8Js;
$called_done = false;
$v8js->done = function () use ( &$called_done ) {
    $called_done = true;
};
$v8js->executeString( 'exit(); PHP.done();' );
assert($called_done === false);

$v8js = new V8Js;
$start = microtime( true );
$v8js->executeString( 'sleep(1);' );
$elapsed = microtime( true ) - $start;
assert( $elapsed > 1 );
