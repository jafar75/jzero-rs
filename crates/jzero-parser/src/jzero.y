%start ClassDecl
%avoid_insert 'IDENTIFIER' 'INTLIT' 'DOUBLELIT' 'STRINGLIT' 'BOOLLIT' 'NULLVAL'

%token 'BREAK' 'DOUBLE' 'ELSE' 'FOR' 'IF' 'INT' 'RETURN' 'VOID' 'WHILE'
%token 'IDENTIFIER' 'CLASSNAME' 'CLASS' 'STRING' 'BOOL'
%token 'INTLIT' 'DOUBLELIT' 'STRINGLIT' 'BOOLLIT' 'NULLVAL'
%token 'LESSTHANOREQUAL' 'GREATERTHANOREQUAL'
%token 'ISEQUALTO' 'NOTEQUALTO' 'LOGICALAND' 'LOGICALOR'
%token 'INCREMENT' 'DECREMENT' 'PUBLIC' 'STATIC'
%token 'LPAREN' 'RPAREN' 'LBRACKET' 'RBRACKET' 'LBRACE' 'RBRACE'
%token 'SEMICOLON' 'COLON' 'COMMA' 'DOT'
%token 'PLUS' 'MINUS' 'STAR' 'SLASH' 'PERCENT'
%token 'ASSIGN' 'NOT' 'LESSTHAN' 'GREATERTHAN'

%%

ClassDecl: 'PUBLIC' 'CLASS' 'IDENTIFIER' ClassBody ;
ClassBody: 'LBRACE' ClassBodyDecls 'RBRACE' | 'LBRACE' 'RBRACE' ;
ClassBodyDecls: ClassBodyDecl | ClassBodyDecls ClassBodyDecl ;
ClassBodyDecl: FieldDecl | MethodDecl | ConstructorDecl ;
FieldDecl: Type VarDecls 'SEMICOLON' ;
Type: 'INT' | 'DOUBLE' | 'BOOL' | 'STRING' | Name ;

Name: 'IDENTIFIER' | QualifiedName ;
QualifiedName: Name 'DOT' 'IDENTIFIER' ;

VarDecls: VarDeclarator | VarDecls 'COMMA' VarDeclarator ;
VarDeclarator: 'IDENTIFIER' | VarDeclarator 'LBRACKET' 'RBRACKET' ;

MethodReturnVal: Type | 'VOID' ;
MethodDecl: MethodHeader Block ;
MethodHeader: 'PUBLIC' 'STATIC' MethodReturnVal MethodDeclarator ;
MethodDeclarator: 'IDENTIFIER' 'LPAREN' FormalParmListOpt 'RPAREN' ;
FormalParmListOpt: FormalParmList | ;
FormalParmList: FormalParm | FormalParmList 'COMMA' FormalParm ;
FormalParm: Type VarDeclarator ;

ConstructorDecl: ConstructorDeclarator Block ;
ConstructorDeclarator: 'IDENTIFIER' 'LPAREN' FormalParmListOpt 'RPAREN' ;

Block: 'LBRACE' BlockStmtsOpt 'RBRACE' ;
BlockStmtsOpt: BlockStmts | ;
BlockStmts: BlockStmt | BlockStmts BlockStmt ;
BlockStmt: LocalVarDeclStmt | Stmt ;

LocalVarDeclStmt: LocalVarDecl 'SEMICOLON' ;
LocalVarDecl: Type VarDecls ;

Stmt: Block | 'SEMICOLON' | ExprStmt | BreakStmt | ReturnStmt
    | IfThenStmt | IfThenElseStmt | IfThenElseIfStmt
    | WhileStmt | ForStmt ;

ExprStmt: StmtExpr 'SEMICOLON' ;

StmtExpr: Assignment | MethodCall | InstantiationExpr ;

IfThenStmt: 'IF' 'LPAREN' Expr 'RPAREN' Block ;
IfThenElseStmt: 'IF' 'LPAREN' Expr 'RPAREN' Block 'ELSE' Block ;
IfThenElseIfStmt: 'IF' 'LPAREN' Expr 'RPAREN' Block ElseIfSequence
    | 'IF' 'LPAREN' Expr 'RPAREN' Block ElseIfSequence 'ELSE' Block ;

ElseIfSequence: ElseIfStmt | ElseIfSequence ElseIfStmt ;
ElseIfStmt: 'ELSE' IfThenStmt ;
WhileStmt: 'WHILE' 'LPAREN' Expr 'RPAREN' Stmt ;

ForStmt: 'FOR' 'LPAREN' ForInit 'SEMICOLON' ExprOpt 'SEMICOLON' ForUpdate 'RPAREN' Block ;
ForInit: StmtExprList | LocalVarDecl | ;
ExprOpt: Expr | ;
ForUpdate: StmtExprList | ;

StmtExprList: StmtExpr | StmtExprList 'COMMA' StmtExpr ;

BreakStmt: 'BREAK' 'SEMICOLON' | 'BREAK' 'IDENTIFIER' 'SEMICOLON' ;
ReturnStmt: 'RETURN' ExprOpt 'SEMICOLON' ;

Primary: Literal | 'LPAREN' Expr 'RPAREN' | FieldAccess | MethodCall ;
Literal: 'INTLIT' | 'DOUBLELIT' | 'BOOLLIT' | 'STRINGLIT' | 'NULLVAL' ;

InstantiationExpr: Name 'LPAREN' ArgListOpt 'RPAREN' ;
ArgListOpt: ArgList | ;
ArgList: Expr | ArgList 'COMMA' Expr ;
FieldAccess: Primary 'DOT' 'IDENTIFIER' ;

MethodCall: Name 'LPAREN' ArgListOpt 'RPAREN'
    | Name 'LBRACE' ArgListOpt 'RBRACE'
    | Primary 'DOT' 'IDENTIFIER' 'LPAREN' ArgListOpt 'RPAREN'
    | Primary 'DOT' 'IDENTIFIER' 'LBRACE' ArgListOpt 'RBRACE' ;

PostFixExpr: Primary | Name ;
UnaryExpr: 'MINUS' UnaryExpr | 'NOT' UnaryExpr | PostFixExpr ;
MulExpr: UnaryExpr | MulExpr 'STAR' UnaryExpr
    | MulExpr 'SLASH' UnaryExpr | MulExpr 'PERCENT' UnaryExpr ;
AddExpr: MulExpr | AddExpr 'PLUS' MulExpr | AddExpr 'MINUS' MulExpr ;
RelOp: 'LESSTHANOREQUAL' | 'GREATERTHANOREQUAL' | 'LESSTHAN' | 'GREATERTHAN' ;
RelExpr: AddExpr | RelExpr RelOp AddExpr ;

EqExpr: RelExpr | EqExpr 'ISEQUALTO' RelExpr | EqExpr 'NOTEQUALTO' RelExpr ;
CondAndExpr: EqExpr | CondAndExpr 'LOGICALAND' EqExpr ;
CondOrExpr: CondAndExpr | CondOrExpr 'LOGICALOR' CondAndExpr ;

Expr: CondOrExpr | Assignment ;
Assignment: LeftHandSide AssignOp Expr ;
LeftHandSide: Name | FieldAccess ;
AssignOp: 'ASSIGN' | 'INCREMENT' | 'DECREMENT' ;

%%