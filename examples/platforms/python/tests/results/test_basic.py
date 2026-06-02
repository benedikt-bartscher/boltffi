import math
from tests.support import DemoTestCase

import demo


class ResultBasicTests(DemoTestCase):
    def test_safe_divide(self) -> None:
        self.demo_case("case:results.basic.safe_divide.should_return_quotient")
        self.assertEqual(demo.safe_divide(10, 2), 5)
        self.demo_case("case:results.basic.safe_divide.should_reject_division_by_zero")
        with self.assertRaises(demo.FfiException) as ctx:
            demo.safe_divide(1, 0)
        self.assertEqual(ctx.exception.args[0], "division by zero")

    def test_safe_sqrt(self) -> None:
        self.demo_case("case:results.basic.safe_sqrt.should_return_square_root")
        self.assertTrue(math.isclose(demo.safe_sqrt(9.0), 3.0, abs_tol=1e-12))
        self.demo_case("case:results.basic.safe_sqrt.should_reject_negative_input")
        with self.assertRaises(demo.FfiException):
            demo.safe_sqrt(-1.0)

    def test_parse_point(self) -> None:
        self.demo_case("case:results.basic.parse_point.should_parse_coordinates")
        point = demo.parse_point("1.0, 2.0")
        self.assertIsInstance(point, demo.Point)
        self.assertTrue(math.isclose(point.x, 1.0, abs_tol=1e-12))
        self.assertTrue(math.isclose(point.y, 2.0, abs_tol=1e-12))
        self.demo_case("case:results.basic.parse_point.should_reject_malformed_input")
        with self.assertRaises(demo.FfiException):
            demo.parse_point("not-a-point")

    def test_always(self) -> None:
        self.demo_case("case:results.basic.always_ok.should_return_doubled_value")
        self.assertEqual(demo.always_ok(21), 42)
        self.demo_case("case:results.basic.always_err.should_return_message_error")
        with self.assertRaises(demo.FfiException) as ctx:
            demo.always_err("boom")
        self.assertEqual(ctx.exception.args[0], "boom")

    def test_divide(self) -> None:
        self.demo_case("case:results.basic.divide.should_return_quotient")
        self.assertEqual(demo.divide(9, 3), 3)
        self.demo_case("case:results.basic.divide.should_reject_division_by_zero")
        with self.assertRaises(demo.FfiException):
            demo.divide(1, 0)

    def test_parse_int(self) -> None:
        self.demo_case("case:results.basic.parse_int.should_parse_integer")
        self.assertEqual(demo.parse_int("42"), 42)
        self.demo_case("case:results.basic.parse_int.should_reject_invalid_integer")
        with self.assertRaises(demo.FfiException):
            demo.parse_int("not-an-int")

    def test_validate_name(self) -> None:
        self.demo_case("case:results.basic.validate_name.should_greet_valid_name")
        self.assertEqual(demo.validate_name("Ada"), "Hello, Ada!")
        self.demo_case("case:results.basic.validate_name.should_reject_empty_name")
        with self.assertRaises(demo.FfiException):
            demo.validate_name("")
