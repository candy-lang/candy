import 'package:freezed_annotation/freezed_annotation.dart';

import '../../lexer/lexer.dart';
import '../../syntactic_entity.dart';
import '../../utils.dart';
import '../../visitor.dart';
import 'declarations.dart';
import 'node.dart';

part 'general.freezed.dart';

@freezed
abstract class CandyFile extends AstNode implements _$CandyFile {
  const factory CandyFile({
    @Default(<UseLine>[]) List<UseLine> useLines,

    /// Only a virtual wrapper around all declarations of this file.
    ModuleDeclaration declaration,
  }) = _CandyFile;
  const CandyFile._();

  @override
  R accept<R>(AstVisitor<R> visitor) => visitor.visitCandyFile(this);

  @override
  Iterable<SyntacticEntity> get children => [...useLines, declaration];
}

@freezed
abstract class UseLine extends AstNode implements _$UseLine {
  const factory UseLine.localAbsolute({
    @Default(<ModifierToken>[]) List<ModifierToken> modifiers,
    @required UseKeywordToken useKeyword,
    @required CrateKeywordToken crateKeyword,
    @Default(<OperatorToken>[]) List<OperatorToken> dots,
    @Default(<IdentifierToken>[]) List<IdentifierToken> pathSegments,
  }) = LocalAbsoluteUseLine;
  const factory UseLine.localRelative({
    @Default(<ModifierToken>[]) List<ModifierToken> modifiers,
    @required UseKeywordToken useKeyword,
    @Default(<OperatorToken>[]) List<OperatorToken> leadingDots,
    @Default(<IdentifierToken>[]) List<IdentifierToken> pathSegments,
    @Default(<OperatorToken>[]) List<OperatorToken> dots,
  }) = LocalRelativeUseLine;
  const factory UseLine.global({
    @Default(<ModifierToken>[]) List<ModifierToken> modifiers,
    @required UseKeywordToken useKeyword,
    @Default(<IdentifierToken>[]) List<IdentifierToken> packagePathSegments,
    @Default(<OperatorToken>[]) List<OperatorToken> slashes,
    OperatorToken dot,
    IdentifierToken moduleName,
  }) = GlobalUseLine;
  const UseLine._();

  @override
  R accept<R>(AstVisitor<R> visitor) => visitor.visitUseLine(this);

  @override
  Iterable<SyntacticEntity> get children => when(
        localAbsolute:
            (modifiers, useKeyword, crateKeyword, dots, pathSegments) => [
          ...modifiers,
          useKeyword,
          crateKeyword,
          ...interleave(dots, pathSegments),
        ],
        localRelative:
            (modifiers, useKeyword, leadingDots, pathSegments, dots) => [
          ...modifiers,
          useKeyword,
          ...leadingDots,
          ...interleave(dots, pathSegments),
        ],
        global: (modifiers, useKeyword, packagePathSegments, slashes, dot,
                moduleName) =>
            [
          ...modifiers,
          useKeyword,
          ...interleave(packagePathSegments, slashes),
          if (dot != null) dot,
          if (moduleName != null) moduleName,
        ],
      );

  bool get isPublic => modifiers.any((m) => m is PublicModifierToken);
}
