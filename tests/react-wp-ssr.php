<?php

/**
 * Get data to load into the `window` object in JS.
 *
 * @return object `window`-compatible object.
 */
function get_window_object() {
	list( $path ) = explode( '?', '/' );
	$port = '80';
	$port = $port !== '80' && $port !== '443' ? (int) $port : '';
	$query = '';
	return [
		'location' => [
			'hash'     => '',
			'host'     => 'localhost',
			'hostname' => 'localhost',
			'pathname' => $path,
			'port'     => $port,
			'protocol' => 'http:',
			'search'   => $query ? '?' . $query : '',
		],
	];
}

// Create stubs.
$window = json_encode( get_window_object() );
$setup = <<<END
// Set up browser-compatible APIs.
var window = this;
Object.assign( window, $window );
window.document = undefined;
var console = {
	warn: print,
	error: print,
	log: ( print => it => print( JSON.stringify( it ) ) )( print )
};
window.setTimeout = window.clearTimeout = () => {};

// Expose more globals we might want.
var global = global || this,
	self = self || this;
var isSSR = true;

// Remove default top-level APIs.
delete exit;
delete var_dump;
delete require;
delete sleep;
END;

$v8 = new V8Js();

/**
 * Filter functions available to the server-side rendering.
 *
 * @param array $functions Map of function name => callback. Exposed on the global `PHP` object.
 * @param string $handle Script being rendered.
 * @param array $options Options passed to render.
 */
// $functions = apply_filters( 'reactwpssr.functions', [], $handle, $options );
$functions = [];
foreach ( $functions as $name => $function ) {
	$v8->$name = $function;
}

// Load the app source.
$source = file_get_contents( __DIR__ . '/react-wp-scripts-bundle.js' );
try {
	// Run the setup.
	$v8->executeString( $setup, 'ssrBootstrap' );

	// Then, execute the script.
	ob_start();
	$v8->executeString( $source, './react-wp-scripts-bundle.js' );
	$output = ob_get_clean();

	echo $output;
} catch ( V8JsScriptException $e ) {
//	handle_exception($e);
	echo $e->getMessage();
}

/**
 * Render JS exception handler.
 *
 * @param V8JsScriptException $e Exception to handle.
 */
function handle_exception( V8JsScriptException $e ) {
	$file = $e->getJsFileName();
	?>
	<style><?php echo file_get_contents( __DIR__ . '/error-overlay.css' ) ?></style>
	<div class="error-overlay"><div class="wrapper"><div class="overlay">
		<div class="header">Failed to render</div>
		<pre class="preStyle"><code class="codeStyle"><?php
			echo esc_html( $file ) . "\n";

			$trace = $e->getJsTrace();
			if ( $trace ) {
				$trace_lines = $error = explode( "\n", $e->getJsTrace() );
				echo esc_html( $trace_lines[0] ) . "\n\n";
			} else {
				echo $e->getMessage() . "\n\n";
			}

			// Replace tabs with tab character.
			$prefix = '> ' . (int) $e->getJsLineNumber() . ' | ';
			echo $prefix . str_replace(
				"\t",
				'<span class="tab">→</span>',
				esc_html( $e->getJsSourceLine() )
			) . "\n";
			echo str_repeat( " ", strlen( $prefix ) + $e->getJsStartColumn() );
			echo str_repeat( "^", $e->getJsEndColumn() - $e->getJsStartColumn() ) . "\n";
			?></code></pre>
		<div class="footer">
			<p>This error occurred during server-side rendering and cannot be dismissed.</p>
			<?php if ( $file === 'ssrBootstrap' ): ?>
				<p>This appears to be an internal error in SSR. Please report it on GitHub.</p>
			<?php elseif ( $file === 'ssrDataInjection' ): ?>
				<p>This appears to be an error in your script's data. Check that your data is valid.</p>
			<?php endif ?>
		</div>
	</div></div></div>
	<?php
}
