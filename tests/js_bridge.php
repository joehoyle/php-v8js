<?php

$v8js = new V8Js;
$result = $v8js->executeString( '({ foo: "bar" })' );
assert($result instanceof V8Object );
assert($result->foo === 'bar' );

$result = $v8js->executeString( '[1, 2, 3]' );
assert($result === [1, 2, 3] );

$result = $v8js->executeString( 'print' );
assert($result === 'Function'); // Todo: better handling for this

$result = $v8js->executeString( '"hello"' );
assert($result === 'hello');

$result = $v8js->executeString( '10' );
assert($result === 10);

$result = $v8js->executeString( '10.1' );
assert($result === 10.1);

$result = $v8js->executeString( 'true' );
assert($result === true);

