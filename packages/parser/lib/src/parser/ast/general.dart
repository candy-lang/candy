import 'package:freezed_annotation/freezed_annotation.dart';

import '../../lexer/lexer.dart';
import '../../syntactic_entity.dart';
import '../../utils.dart';
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
  Iterable<SyntacticEntity> get children => [...useLines, declaration];
}

@freezed
abstract class UseLine extends AstNode implements _$UseLine {
  const factory UseLine.localAbsolute({
    @required UseKeywordToken useKeyword,
    @required CrateKeywordToken crateKeyword,
    @Default(<OperatorToken>[]) List<OperatorToken> dots,
    @Default(<IdentifierToken>[]) List<IdentifierToken> pathSegments,
  }) = LocalAbsoluteUseLine;
  const factory UseLine.localRelative({
    @required UseKeywordToken useKeyword,
    @Default(<OperatorToken>[]) List<OperatorToken> leadingDots,
    @Default(<IdentifierToken>[]) List<IdentifierToken> pathSegments,
    @Default(<OperatorToken>[]) List<OperatorToken> dots,
  }) = LocalRelativeUseLine;
  const factory UseLine.global({
    @required UseKeywordToken useKeyword,
    @Default(<IdentifierToken>[]) List<IdentifierToken> packagePathSegments,
    @Default(<OperatorToken>[]) List<OperatorToken> slashes,
    OperatorToken dot,
    IdentifierToken moduleName,
  }) = GlobalUseLine;
  const UseLine._();

  @override
  Iterable<SyntacticEntity> get children => when(
        localAbsolute: (useKeyword, crateKeyword, dots, pathSegments) => [
          useKeyword,
          crateKeyword,
          ...interleave(dots, pathSegments),
        ],
        localRelative: (useKeyword, leadingDots, pathSegments, dots) => [
          useKeyword,
          ...leadingDots,
          ...interleave(dots, pathSegments),
        ],
        global: (useKeyword, packagePathSegments, slashes, dot, moduleName) => [
          useKeyword,
          ...interleave(packagePathSegments, slashes),
          if (dot != null) dot,
          if (moduleName != null) moduleName,
        ],
      );
}
