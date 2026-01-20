The bridge only supports a single Core IR and can not link in other axis core IRs in this release.  The workaround is to put all the axis code files as sources to the compiler.

The lexer/parser is not 100% compliant with the Axis Language Specification. Some constructs may be missing or behave differently than specified.  This will be rectified in the next release with the focus on multiple surface languages.

There is no support for windows helper scripts in this release.  This is a deliberate scope reduction to focus on core functionality.  Windows support will be added in a future release.  Also note that the system has not been tested on Windows.