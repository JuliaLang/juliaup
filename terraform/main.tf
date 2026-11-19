# IAM role assumed by this repository's GitHub Actions workflows (release.yml and
# updateversiondb.yml) through GitHub's OIDC provider to upload to S3. The provider itself
# already exists in the account and is shared with other repositories, so it is only looked
# up here, not managed.

locals {
  repo = "JuliaLang/juliaup"

  # Release artifacts, version db and channel version files (AWS_S3_BUCKET in the workflows)
  artifact_bucket = "julialang2"
  artifact_prefix = "juliaup"

  # Install scripts and Windows installers (AWS_S3_SCRIPT_BUCKET in the workflows)
  script_bucket = "juliaup-install"

  # Every job that uploads to S3 runs in one of these GitHub environments, so the trust
  # policy is keyed on the environment rather than on the ref. The environments' deployment
  # branch/tag rules are what restrict which refs can assume the role.
  environments = [
    "package",
    "dev-channel",
    "release-preview-channel",
    "release-channel",
  ]
}

data "aws_iam_openid_connect_provider" "github" {
  url = "https://token.actions.githubusercontent.com"
}

data "aws_iam_policy_document" "deploy_trust" {
  statement {
    actions = ["sts:AssumeRoleWithWebIdentity"]
    principals {
      type        = "Federated"
      identifiers = [data.aws_iam_openid_connect_provider.github.arn]
    }
    condition {
      test     = "StringEquals"
      variable = "token.actions.githubusercontent.com:aud"
      values   = ["sts.amazonaws.com"]
    }
    condition {
      test     = "StringEquals"
      variable = "token.actions.githubusercontent.com:sub"
      values   = [for env in local.environments : "repo:${local.repo}:environment:${env}"]
    }
  }
}

# The workflows only run `aws s3 sync` from local directories (without --delete), which
# needs ListBucket to diff against the destination and PutObject to upload.
data "aws_iam_policy_document" "deploy" {
  statement {
    sid       = "ListArtifactPrefix"
    actions   = ["s3:ListBucket"]
    resources = ["arn:aws:s3:::${local.artifact_bucket}"]
    condition {
      test     = "StringLike"
      variable = "s3:prefix"
      values   = [local.artifact_prefix, "${local.artifact_prefix}/*"]
    }
  }
  statement {
    sid = "UploadArtifacts"
    # PutObjectAcl because the uploads use --acl public-read
    actions   = ["s3:PutObject", "s3:PutObjectAcl"]
    resources = ["arn:aws:s3:::${local.artifact_bucket}/${local.artifact_prefix}/*"]
  }
  statement {
    sid       = "ListScriptBucket"
    actions   = ["s3:ListBucket"]
    resources = ["arn:aws:s3:::${local.script_bucket}"]
  }
  statement {
    sid = "UploadScripts"
    # No PutObjectAcl: the bucket has ACLs disabled (BucketOwnerEnforced) and is made public
    # through its bucket policy
    actions   = ["s3:PutObject"]
    resources = ["arn:aws:s3:::${local.script_bucket}/*"]
  }
}

resource "aws_iam_role" "deploy" {
  name               = "juliaup-deploy"
  description        = "Upload juliaup release artifacts, install scripts and version db for JuliaLang/juliaup CI"
  assume_role_policy = data.aws_iam_policy_document.deploy_trust.json
  # The lowest value IAM allows. The workflows request 900s (role-duration-seconds), the
  # lowest STS allows; uploads take well under a minute.
  max_session_duration = 3600
}

resource "aws_iam_role_policy" "deploy" {
  name   = "s3-deploy-juliaup"
  role   = aws_iam_role.deploy.id
  policy = data.aws_iam_policy_document.deploy.json
}

output "deploy_role_arn" {
  value = aws_iam_role.deploy.arn
}
